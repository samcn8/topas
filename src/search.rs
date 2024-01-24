//! This module contains all functionality related to searching a
//! the chess board.  The core of this functionality is a negamax
//! implementation with alpha-beta pruning.
//! 
//! A transposition table (TT) is used to store results of previously
//! searched nodes.  The TT size is configurable.  The TT is stored
//! on the heap (in a Vec) and so should be sized with respect to the
//! available memory on the system.
//! 
//! This module uses iterative deepening to progressively search higher
//! depths, storing the principal variation (PV) -- the best line
//! computed so far -- with the first move of the PV being the best
//! candidate move for the next depth.

use std::time;
use std::cmp;
use std::mem;
use std::sync::mpsc::Receiver;
use crate::evaluate;
use crate::chess_board;
use crate::movegen;
use crate::pieces;
use crate::bitboard;

// Default size of transposition table in MB
pub const DEFAULT_TT_SIZE_MB: u64 = 16;

// Scores for terminal states and infinity
const CHECKMATE_VALUE: i32 = 50000;
const DRAW_VALUE: i32 = 0;
const INF: i32 = 100000000;

// Initial aspiration window size (half)
const ASPIRATION_WINDOW_HALF_SIZE: i32 = 30;

// When prioritizing moves, a bonus may be assigned to a move.
// Principal variation (PV) moves are the most valuable, and are
// usually discovered on the previous iterative deepening loop.
// Moves that lead to a beta cutoff are also very valuable as they
// can signficantly decrease the search space.  Promotions and
// captures are valuable, followed by killer moves.
const PV_MOVE_PRIORITY_BONUS: i32 = 600;
const CUTOFF_PRIORITY_BONUS: i32 = 500;
const PROMOTION_PRIORITY_BONUS: i32 = 400;
const CAPTURE_PRIORITY_BONUS: i32 = 300;
const KILLER_MOVE_BONUS: i32 = 200;
const PAWN_PUSH_BONUS: i32 = 100;

// Piece values in centipawns used in static exchange evaluation (SEE)
// Indexed by PNBRQK position.
const SEE_PIECE_VALUES: [i32; 6] = [100, 300, 300, 500, 900, 20000];

// How frequently (in number of function calls of negamax) to check for
// a halt condition.  A halt can happen if we run out of time or a
// stop command was issued.
const CHECK_HALT_CONDITION_INTERVAL: u64 = 5000;

// TT Flag corresponding to a value
enum TTFlag {

    // An exact value is one that falls between alpha and beta and
    // represents a PV move
    Exact,

    // A lower bound value is one that failed high and caused a
    // beta-cutoff -- the true value is at least as good as this.
    Lowerbound,

    // An upper bound value is one that failed low, meaning it didn't
    // rise to the level of an already found acceptable move.
    Upperbound,
}

// An entry within a transposition table.  The total size of an
// entry is 24B.
struct TTEntry {

    // Zobrist hash of the board state at this node, used to
    // ensure we didn't have a hash table collision
    zobrist_hash: u64,

    // The depth of the search at this node.  Note that this is
    // the "depth left to search when we first hit this node" as
    // opposed to "the ply we were at when searching this node".
    // During a search, the depth starts high (at the root node)
    // and hits 0 at the end of the standard search.
    depth: u8,

    // The score at this node (caveated by the flag)
    value: i32,

    // The flag indicating whether this score is exact, and upper
    // bound, or a lower bound, according to the alpha-beta search
    flag: TTFlag,

    // The best move discovered at this node / game state.  Note
    // that if this is an "exact" node, then this is a PV move.
    // If this is a "lower bound" node, then this represents a
    // beta-cutoff move -- a move that is too good.  If this is a
    // "upper bound", then there is no best move -- this field
    // should be ignored.
    // This represents (start square, end square).
    best_move: Option<(u8, u8)>,

    // Whether or not this TT entry is still valid
    valid: bool,

}

// An entry into a static exchange evaluation (SEE) attack vector.
struct SEEAttacker {

    // The value of the piece attacking
    value: i32,

    // The location of the piece attacking
    square: usize,

    // A list of square locations of any blockers
    blockers: Vec<usize>,

}

// Information about the top move discovered from a search depth
#[derive(Debug)]
pub struct BestMoveInformation {

    // The best move represented as (start square, end square)
    pub best_move_from_last_iteration: Option<(u8, u8)>,

    // The value / score from the engine's (current player's)
    // perspective assuming the best move is played
    pub value: i32,

    // The max depth that ended up being searched in the iterative
    // deepening loop
    pub depth_searched: u8,

    // The total moves that ended up being searched
    pub moves_analyzed: i32,

    // Time in milliseconds that it took to find this move
    pub duration_of_search: u128,

    // The PV line computed (start square, end square, promotion piece if it exists)
    pub pv_line: Vec<(u8, u8, Option<usize>)>,

}

// The main engine
pub struct SearchEngine {

    // The receiving channel used to communicate UCI commands
    // to the engine thread.
    pub rx_channel: Receiver<String>,

    // The game board
    board: chess_board::ChessBoard,

    // The transposition table size in entries.  Each entry
    // is 24B so the total size of the TT is: 24B * num_tt_entries.
    num_tt_entries: usize,

    // The transposition table
    transposition_table: Vec<Option<TTEntry>>,

    // Killer moves, indexed by ply.  We assume that we will
    // never reach greater than 100 ply.
    primary_killers: [Option<(u8, u8)>; 100],
    secondary_killers: [Option<(u8, u8)>; 100],

    // Max depth we were instructed to search to
    max_depth_for_search: u8,
    
    // The stored best move from the last iteration
    // represented by (start square, end square)
    best_move_from_last_iteration: Option<(u8, u8)>,

    // Total moves analyzed in current search
    moves_analyzed: i32,

    // The maximum time we can spend on this move in milliseconds
    time_max_for_move: u128,

    // The time we started the move
    move_start_time: time::Instant,

    // Whether the current iteration was halted due to running out of time
    // or receiving a stop command
    halt_search: bool,

    // Count down until checking for a halt condition
    halt_check_countdown: u64,

}

impl SearchEngine {

    // Construct a new SearchEngine
    pub fn new(rx: Receiver<String>) -> SearchEngine {
        SearchEngine {
            rx_channel: rx,
            board: chess_board::ChessBoard::new(),
            num_tt_entries: (DEFAULT_TT_SIZE_MB * 1000000 / mem::size_of::<TTEntry>() as u64) as usize,
            transposition_table: Vec::new(),
            primary_killers: [None; 100],
            secondary_killers: [None; 100],
            max_depth_for_search: 0,
            best_move_from_last_iteration: None,
            moves_analyzed: 0,
            time_max_for_move: 0,
            move_start_time: time::Instant::now(),
            halt_search: false,
            halt_check_countdown: CHECK_HALT_CONDITION_INTERVAL,
        }
    }

    // Start a new game, resetting everything
    pub fn new_game(&mut self) {

        // Reset the board
        self.board.new_game();

        // Reset the transposition table
        self.transposition_table.clear();
        self.transposition_table.resize_with(self.num_tt_entries, ||-> Option<TTEntry> {None});

        // Reset other state
        self.best_move_from_last_iteration = None;
        self.moves_analyzed = 0;
    
    }

    // Sets the position of the board.  Since the UCI protocol is stateless,
    // we'll typically reset the board state after each search.
    // The fen string is in FEN format.  See:
    // https://en.wikipedia.org/wiki/Forsyth–Edwards_Notation
    // The move string is in long algebraic notation without piece names,
    // with spaces between each move, as dictated by the UCI protocol.
    // move.  For instance, "e2e4 b8c6".  Promotion looks like "f7f8q".
    pub fn set_board_state(&mut self, fen_str: &str, move_str: &str) {

        // Get the list of moves passed in
        let moves = movegen::convert_moves_str_into_list(move_str);

        // Start the board at a given starting position
        self.board.new_game_from_fen(fen_str);

        // Play out the provided moves
        for (start_square, end_square, promotion) in moves {
            self.board.make_move(start_square, end_square, promotion);
        }

        // Reset other state
        self.best_move_from_last_iteration = None;
        self.moves_analyzed = 0;

    }

    // Sets the transposition table size in MB
    pub fn set_tt_size_mb(&mut self, size_mb: u64) {
        self.num_tt_entries = (size_mb * 1000000 / mem::size_of::<TTEntry>() as u64) as usize;
        self.transposition_table.resize_with(self.num_tt_entries, ||-> Option<TTEntry> {None});
    }

    // Returns the color of the player to move
    pub fn color_turn(&self) -> usize {
        if self.board.whites_turn {pieces::COLOR_WHITE} else {pieces::COLOR_BLACK}
    }

    // Prints the board, for debugging purposes
    pub fn print_board(&self) {
        self.board.print_debug();
    }

    // This returns the engine's top move given a maximum search depth.
    // This uses self.board as the current state of the board to search from.
    // This uses an iterative deepening search.  The PV move found in the
    // previous iteration is the first searched node in the next iteration.
    // This will print information to standard out in UCI format in compliance
    // with the UCI protocol.
    pub fn find_best_move(&mut self, mut max_depth: u8, time_available: i32, time_inc: i32, moves_to_go: u16) {

        // Sanity check on transposition tables.  Note that the user should
        // have sent a ucinewgame command first to reset the transposition
        // tables.  But, if they did not, we'll reset them here so we don't
        // crash.
        if self.transposition_table.len() == 0 {
            self.transposition_table.clear();
            self.transposition_table.resize_with(self.num_tt_entries, ||-> Option<TTEntry> {None});
        }

        // If depth is 0, then we're not using depth as a limiter
        if max_depth == 0 {
            max_depth = 99;
        }

        // If time_available is greater than 0, then we're using
        // time as a limiter
        let mut time_for_move = INF;
        if time_available > 0 {

            // For time management purposes, we allocate time into
            // equal chunks based on the number of moves to go, and
            // add this chunk to half of our time increment.  If we're
            // running low on time, we try to ensure we have at least
            // 100ms for a move.
            time_for_move = time_available / (moves_to_go as i32) + time_inc / 2;
            if time_for_move > time_available {
                time_for_move = time_available - 500;
            }
            if time_for_move < 0 {
                time_for_move = 100;
            }
        }

        // Update start time and move time
        self.move_start_time =  time::Instant::now();
        self.time_max_for_move = time_for_move as u128;
        self.halt_check_countdown = CHECK_HALT_CONDITION_INTERVAL;

        // Information about the last iteration
        let mut last_iteration_info: Option<BestMoveInformation> = None;

        // Start the clock for the first iteration
        let mut start_time_iteration = time::Instant::now();
        
        // Start of iterative deepening loop
        let mut value: i32;
        let mut depth = 1;
        let mut alpha = -INF;
        let mut beta = INF;
        let mut left_aspiration_window = ASPIRATION_WINDOW_HALF_SIZE;
        let mut right_aspiration_window = ASPIRATION_WINDOW_HALF_SIZE;
        while depth <= max_depth {

            // Store the max depth for this search
            self.max_depth_for_search = depth;

            // Find the best move using negamax
            value = self.negamax(depth, alpha, beta, true);

            // Check if this search was halted due to time or a stop command
            // and if so then ignore the results
            if self.halt_search {
                break;
            }

            // If our evaluation fell outside of the window, we have
            // to redo the search at the same depth with a wider window
            if value <= alpha {
                alpha -= left_aspiration_window;
                left_aspiration_window *= 2;
                self.best_move_from_last_iteration = None;
                continue;
            } else if value >= beta {
                beta += right_aspiration_window;
                right_aspiration_window *= 2;
                self.best_move_from_last_iteration = None;
                continue;
            }

            // End the clock for this iteration
            let duration_iteration = start_time_iteration.elapsed();

            // Create a record for the iteration
            let info = BestMoveInformation {
                best_move_from_last_iteration: self.best_move_from_last_iteration,
                value,
                moves_analyzed: self.moves_analyzed,
                depth_searched: depth,
                duration_of_search: duration_iteration.as_millis(),
                pv_line: self.extract_pv_line(),
            };

            // Per the UCI protocol, print "info" messages to standard out
            println!("info depth {} score cp {} nodes {} time {} pv {}",
                info.depth_searched,
                info.value,
                info.moves_analyzed,
                info.duration_of_search,
                movegen::convert_move_list_to_lan(&info.pv_line));

            // Store the record
            last_iteration_info = Some(info);

            // Reset the aspiration window for the next depth
            left_aspiration_window = ASPIRATION_WINDOW_HALF_SIZE;
            right_aspiration_window = ASPIRATION_WINDOW_HALF_SIZE;
            alpha = value - left_aspiration_window;
            beta = value + right_aspiration_window;

            // Reset some state for next iteration
            self.best_move_from_last_iteration = None;
            self.moves_analyzed = 0;

            // Don't start the next iteration if we don't have sufficient time.
            // We assume it will take at least 2x longer to search the next depth
            // compare to the depth just searched.
            if self.move_start_time.elapsed().as_millis() + 2 * duration_iteration.as_millis() > self.time_max_for_move {
                break;
            }

            // Increase depth
            depth += 1;

            // Start the clock for the next iteration
            start_time_iteration = time::Instant::now();

        }

        // Clear out the transposition tables and search-specific state
        self.transposition_table.clear();
        self.transposition_table.resize_with(self.num_tt_entries, ||-> Option<TTEntry> {None});
        self.halt_search = false;
        self.time_max_for_move = 0;
        self.halt_check_countdown = CHECK_HALT_CONDITION_INTERVAL;
        self.primary_killers = [None; 100];
        self.secondary_killers = [None; 100];
        self.max_depth_for_search = 0;

        // Get the best move
        let mut bm = String::from("0000");
        if let Some(info) = &last_iteration_info {
            if let Some((move_start, move_end)) = info.best_move_from_last_iteration {
                
                // If pawn promotion, add the promotion piece
                // TODO: Allow promotions to pieces other than queens.
                let mut promotion = None;
                if move_end >= 56 || move_end <= 7 {
                    if let Some((_,p)) = self.board.get_color_and_piece_on_square(move_start as usize) {
                        if p == pieces::PAWN {
                            promotion = Some(pieces::QUEEN);
                        }
                    }
                }
            
                let move_vec = vec!((move_start, move_end, promotion));
                bm = movegen::convert_move_list_to_lan(&move_vec);

            }
        }

        // Per the UCI protocol, print the best move to standard out
        println!("bestmove {}", bm);

    }

    // This returns a priority bonus for move ordering if the move is
    // a PV move or causes a beta cutoff (in other words, is a hash move).
    // This is determined via lookup in the transposition table.
    fn get_move_priority_bonus(&self, start_square: usize, end_square: usize) -> i32 {
        let tt_key = (self.board.zobrist_hash % self.num_tt_entries as u64) as usize;
        if let Some(tt_entry) = &self.transposition_table[tt_key] {
            if tt_entry.valid && tt_entry.zobrist_hash == self.board.zobrist_hash {
                if let Some((bm_start_square, bm_end_square)) = tt_entry.best_move {
                    if bm_start_square == start_square as u8 && bm_end_square == end_square as u8 {
                        match tt_entry.flag {
                            TTFlag::Exact => return PV_MOVE_PRIORITY_BONUS,
                            TTFlag::Lowerbound => return CUTOFF_PRIORITY_BONUS,
                            TTFlag::Upperbound => return 0,
                        }
                    }
                }
            }
        }
        0
    }

    // Add any "blockers" on the specified ray, except for the target.
    // Used for SEE attacker computation.
    fn add_blocks_to_see_attacker(&self, capture_square: usize, ray: u64, blockers: &mut Vec<usize>) {
        for s in bitboard::occupied_squares(ray & self.board.bb_occupied_squares) {
            if s != capture_square {
                blockers.push(s);
            }
        }
    }

    // Check all four directional "rays" starting from our position to see
    // if we hit the target, assuming no blocking pieces exist.  Return true
    // if we did, false if not.  Along the way, collect any blockers for the
    // ray (if there is one) that hits the target.
    fn check_bishop_for_see_attack(&self, square: usize, capture_square: usize, capture_square_bb: u64, blockers: &mut Vec<usize>) -> bool {
        let mut test_ray = movegen::get_diagonal_attacks_bb(capture_square_bb, square, 1);
        if test_ray & capture_square_bb != 0 {
            self.add_blocks_to_see_attacker(capture_square, test_ray, blockers);
            return true;
        }
        test_ray = movegen::get_diagonal_attacks_bb(capture_square_bb, square, 2);
        if test_ray & capture_square_bb != 0 {
            self.add_blocks_to_see_attacker(capture_square, test_ray, blockers);
            return true;
        }
        test_ray = movegen::get_antidiagonal_attacks_bb(capture_square_bb, square, 1);
        if test_ray & capture_square_bb != 0 {
            self.add_blocks_to_see_attacker(capture_square, test_ray, blockers);
            return true;
        }
        test_ray = movegen::get_antidiagonal_attacks_bb(capture_square_bb, square, 2);
        if test_ray & capture_square_bb != 0 {
            self.add_blocks_to_see_attacker(capture_square, test_ray, blockers);
            return true;
        }
        return false;
    }

    // Check all four directional "rays" starting from our position to see
    // if we hit the target, assuming no blocking pieces exist.  Return true
    // if we did, false if not.  Along the way, collect any blockers for the
    // ray (if there is one) that hits the target.
    fn check_rook_for_see_attack(&self, square: usize, capture_square: usize, capture_square_bb: u64, blockers: &mut Vec<usize>) -> bool {
        let mut test_ray = movegen::get_file_attacks_bb(capture_square_bb, square, 1);
        if test_ray & capture_square_bb != 0 {
            self.add_blocks_to_see_attacker(capture_square, test_ray, blockers);
            return true;
        }
        test_ray = movegen::get_file_attacks_bb(capture_square_bb, square, 2);
        if test_ray & capture_square_bb != 0 {
            self.add_blocks_to_see_attacker(capture_square, test_ray, blockers);
            return true;
        }
        test_ray = movegen::get_rank_attacks_bb(capture_square_bb, square, 1);
        if test_ray & capture_square_bb != 0 {
            self.add_blocks_to_see_attacker(capture_square, test_ray, blockers);
            return true;
        }
        test_ray = movegen::get_rank_attacks_bb(capture_square_bb, square, 2);
        if test_ray & capture_square_bb != 0 {
            self.add_blocks_to_see_attacker(capture_square, test_ray, blockers);
            return true;
        }
        return false;
    }

    // Perform static exchange evaluation (SEE) for a particular capture move.
    // To keep this as fast as possible, this will evaluate the capture exchanges
    // without checking if moves are legal (e.g., it will consider an illegal
    // move that puts your king in check).  This will return a score for the capture.
    // Scores greater than or equal to 0 are worth searching further because they
    // could be winning captures.  Scores less than 0 are likely loosing captures
    // and hence less worthy of further search.
    // Note that only a simulation is performed here; we do not actually "make_move".
    fn see_capture_eval(&self, capture_move: &movegen::ChessMove) -> i32 {

        // Extract captured piece
        let cap_piece = if let Some(c) = capture_move.captured_piece {
            c
        } else {
            panic!("Attempting SEE on non-capture move")
        };

        // TODO - factor in en passant movement.  For now, we're going to just
        // be safe and assume all en passant captures are worth searching.
        if capture_move.is_en_passant {
            return 1;
        }

        // Create a bitboard representing the capture square
        let capture_square_bb = bitboard::to_bb(capture_move.end_square);

        // Make a list of all attackers of the target square,
        // even if there are nodes blocking the way (for sliding piece
        // attackers).   We will note those blockers.  We will then simulate
        // the attack sequence using the least valuable piece remaining for
        // each side, removing potential blockers as we go, until there are
        // no more attackers left.
        // The following vector contains two other vectors -- one for white
        // and one for black.  Each of these color vectors is a list of
        // SEE_Attacker entries which indicate the piece value, square of
        // the piece, and any blockers.
        let mut attackers = vec![Vec::new(), Vec::new()];
        for color in 0..2 {
            for (piece, bb) in self.board.bb_pieces[color].iter().enumerate() {
                for square in bitboard::occupied_squares(*bb) {

                    // Skip the initial capture; we'll simulate that
                    // seperately to kick things off.
                    if square == capture_move.start_square {
                        continue;
                    }

                    // Store blockers
                    let mut blockers = Vec::new();

                    // Get the attack bitboard of the appropriate piece
                    let mut capture_attacker_bb = 0;
                    if piece == pieces::PAWN {
                        capture_attacker_bb = bitboard::BB_PAWN_ATTACKS[color][square];
                    } else if piece == pieces::KNIGHT {
                        capture_attacker_bb = bitboard::BB_KNIGHT_ATTACKS[square]
                    } else if piece == pieces::BISHOP {
                        if self.check_bishop_for_see_attack(square, capture_move.end_square, capture_square_bb, &mut blockers) {
                            // We only have to indicate that we've attacked the square
                            capture_attacker_bb = capture_square_bb;
                        }
                    } else if piece == pieces::ROOK {
                        if self.check_rook_for_see_attack(square, capture_move.end_square, capture_square_bb, &mut blockers) {
                            // We only have to indicate that we've attacked the square
                            capture_attacker_bb = capture_square_bb;
                        }
                    } else if piece == pieces::QUEEN {
                        if self.check_bishop_for_see_attack(square, capture_move.end_square, capture_square_bb, &mut blockers) {
                            // We only have to indicate that we've attacked the square
                            capture_attacker_bb = capture_square_bb;

                        } else if self.check_rook_for_see_attack(square, capture_move.end_square, capture_square_bb, &mut blockers) {
                            // We only have to indicate that we've attacked the square
                            capture_attacker_bb = capture_square_bb;
                        }
                    } else if piece == pieces::KING {
                        capture_attacker_bb = bitboard::BB_KING_ATTACKS[square]
                    }

                    // Determine if the piece is attacking the captured square.
                    // Note that bitbord is 0 otherwise, which will bypass
                    // this if statement.
                    if capture_attacker_bb & capture_square_bb != 0 {
                        attackers[color].push(SEEAttacker {
                            value: SEE_PIECE_VALUES[piece],
                            square,
                            blockers,
                        });
                    }

                }
            }
        }

        // Sort the attackers from least to most valuable (we're always going
        // to attack with the least valuable piece first.
        attackers[pieces::COLOR_WHITE].sort_unstable_by(|a, b| a.value.cmp(&b.value));
        attackers[pieces::COLOR_BLACK].sort_unstable_by(|a, b| a.value.cmp(&b.value));

        // Simulate the initial capture
        let my_color = if self.board.whites_turn {pieces::COLOR_WHITE} else {pieces::COLOR_BLACK};
        let mut current_turn_color = my_color;
        let mut scores = Vec::new();
        let mut score = SEE_PIECE_VALUES[cap_piece];
        let mut attacked_piece_value = SEE_PIECE_VALUES[capture_move.piece];
        scores.push(score);
        let mut selected_attacker_square = Some(capture_move.start_square);
        let mut selected_attacker_value = SEE_PIECE_VALUES[capture_move.piece];

        // Switch turns in the simulation
        current_turn_color = 1 - current_turn_color;

        // Simulate captures one by one until there are none left
        loop {

            // Remove the attacker as a blocker
            if let Some(sa) = selected_attacker_square {
                for color in 0..2 {
                    for i in attackers[color].iter_mut() {
                        if let Some(pos) = i.blockers.iter().position(|x| *x == sa) {
                            i.blockers.remove(pos);
                        }
                    }
                }
            }

            // Get the next non-blocked attacker
            selected_attacker_square = None;
            let mut attacker_pos = 0;
            for i in attackers[current_turn_color].iter() {
                if i.blockers.is_empty() {
                    selected_attacker_square = Some(i.square);
                    selected_attacker_value = i.value;
                    break;
                }
                attacker_pos += 1;
            }

            // If we couldn't find a suitable attacker, we're done
            if selected_attacker_square.is_none() {
                break;
            }

            // Update the score with this attack and remove the attacker
            if let Some(l) = scores.last() {
                score = attacked_piece_value - l;
            } else {
                panic!("Cannot find last score");
            }
            attacked_piece_value = selected_attacker_value;
            scores.push(score);
            attackers[current_turn_color].remove(attacker_pos);
            current_turn_color = 1 - current_turn_color;

        }

        // Finally, evaluate the scores, taking into account the option for a
        // player to refuse to continue the capture line, and return in
        // centipawns
        for i in (1..scores.len()).rev() {
            if scores[i-1] > -scores[i] {
                scores[i-1] = -scores[i];
            }
        }
        scores[0]
        
    }

    // This scores moves, assigning a priority (higher is better)
    // Priority from high to low is: (1) PV moves, (2) moves that cause
    // a beta cut-off, (3) captures, sorted by MVV-LVA, (4) killer moves,
    // and (5) all other moves.
    fn score_moves(&self, moves: &mut Vec<movegen::ChessMove>, ply: u8) {

        // Assign a priority to all moves
        for m in moves.iter_mut() {

            // Check the transposition table for PV and cut-off moves
            let mut priority = self.get_move_priority_bonus(m.start_square, m.end_square);

            // Check for promotions, captures, and killer moves
            if priority == 0 {
                if m.piece == pieces::PAWN && (m.end_square / 8 == 0 || m.end_square / 8 == 7) {
                    priority = PROMOTION_PRIORITY_BONUS;
                } else if let Some(cap) = m.captured_piece {
                    priority = CAPTURE_PRIORITY_BONUS + pieces::MVV_LVA[cap][m.piece];
                } else {
                    let cur_move = Some((m.start_square as u8, m.end_square as u8));
                    if cur_move == self.primary_killers[ply as usize] || cur_move == self.secondary_killers[ply as usize] {
                        priority = KILLER_MOVE_BONUS;
                    }
                }
            }

            // Give a slight edge to pawn pushes
            if priority == 0 && m.piece == pieces::PAWN {
                priority = PAWN_PUSH_BONUS;
            }

            // Set priority
            m.priority = priority;

        }

    }

    // Place the ith highest priority move in the ith slot.
    // Values in positions less than i have already been sorted / used so
    // we only have to check moves at and to the right of i.
    fn sort_move_with_priority(&self, moves: &mut Vec<movegen::ChessMove>, i: usize) {
        let mut highest_value = moves[i].priority;
        let mut highest_index = i;
        for index in (i+1)..moves.len() {
            let p = moves[index].priority;
            if p > highest_value {
                highest_value = p;
                highest_index = index;
            }
        }
        if highest_index != i {
            moves.swap(i, highest_index);
        }
    }

    // This is an implementation of the quiescence search, which allows
    // the engine to keep searching "non-quiet" (i.e, capture) moves
    // beyond the search horizon.  This is done to mitigate the horizon
    // effect, which may cause a bad decision to be made right at the edge
    // of the search horizon.
    // See https://www.chessprogramming.org/Quiescence_Search
    fn quiesce(&mut self, mut alpha: i32, beta: i32) -> i32 {
        
        // Before doing any searching, check to make sure we're not
        // halting.  For performance reasons, we won't check this
        // condition on every quiesce call.
        if self.halt_search {
            return 0;
        }
        self.halt_check_countdown -= 1;
        if self.halt_check_countdown <= 0 {
            self.halt_check_countdown = CHECK_HALT_CONDITION_INTERVAL;

            // Check if we should halt due to time
            if self.move_start_time.elapsed().as_millis() > self.time_max_for_move {
                self.halt_search = true;
                return 0;
            }

            // Check if we should halt due to a stop command
            for cmd in self.rx_channel.try_iter() {
                if cmd.trim() == "stop" {
                    self.halt_search = true;
                    return 0;
                }
            }

        }

        // This is our stand pat score, which is the current score
        // of the board without additional moves.
        let stand_pat = evaluate::static_evaluation(&self.board);

        // Check for a beta cut-off
        if stand_pat >= beta {
            return beta;
        }

        // Delta pruning
        // See https://www.chessprogramming.org/Delta_Pruning
        if stand_pat < alpha - pieces::PIECE_VALUES_MG[pieces::QUEEN] {
            return alpha;
        }

        // Increase alpha if our stand pat score is high enough
        if alpha < stand_pat {
            alpha = stand_pat;
        }

        // Generate all legal moves.  Note that we will only search
        // capture moves.
        let my_color = if self.board.whites_turn {pieces::COLOR_WHITE} else {pieces::COLOR_BLACK};
        let mut moves = movegen::generate_all_psuedo_legal_moves(&self.board, my_color, true);

        // Assign priorities according to MVV-LVA
        for m in moves.iter_mut() {
            if m.captured_piece.is_none() {
                println!("ERROR: Non-capture selected in q search");
                continue;
            }
            m.priority = pieces::MVV_LVA[m.captured_piece.unwrap()][m.piece];
        }

        // Recursively search the capture moves
        for i in 0..moves.len() {

            // Grab the next highest priority move
            self.sort_move_with_priority(&mut moves, i);
            let m = &moves[i];

            // Check to make sure it's legal
            if !movegen::is_legal_move(&mut self.board, m) {
                continue;
            }

            // Update analyzed moves
            self.moves_analyzed += 1;

            // Perform static exchange evaluation on this capture
            // move to determine if it's worth searching further.
            if self.see_capture_eval(m) < 0 {
                continue;
            }

            // Make the move
            self.board.make_move(m.start_square, m.end_square, None);

            // Recursively search on the new board state
            let score_for_move = -self.quiesce(-beta, -alpha);

            // Unmake the move
            self.board.unmake_move();

            // Check for a beta cut-off
            if score_for_move >= beta {
                return beta;
            }

            // Check to see if we can raise alpha
            if score_for_move > alpha {
                alpha = score_for_move;
            }

        }

        // Return alpha, the minimum score we know we can get
        alpha

    }

    // This is an implementation of the minimax algorithm with alpha-beta
    // pruning and is the core of the engine's search routine.  This uses
    // transposition table lookups to enhance performance.
    // See https://en.wikipedia.org/wiki/Negamax
    fn negamax(&mut self, depth: u8, mut alpha: i32, beta: i32, root: bool) -> i32 {
        
        // Before doing any searching, check to make sure we're not
        // halting.  For performance reasons, we won't check this
        // condition on every negamax call.
        if self.halt_search {
            return 0;
        }
        self.halt_check_countdown -= 1;
        if self.halt_check_countdown <= 0 {
            self.halt_check_countdown = CHECK_HALT_CONDITION_INTERVAL;

            // Check if we should halt due to time
            if self.move_start_time.elapsed().as_millis() > self.time_max_for_move {
                self.halt_search = true;
                return 0;
            }

            // Check if we should halt due to a stop command
            for cmd in self.rx_channel.try_iter() {
                if cmd.trim() == "stop" {
                    self.halt_search = true;
                    return 0;
                }
            }

        }

        // Update moves analyzed count
        self.moves_analyzed += 1;

        // Check transposition tables for any cached values
        let alpha_orig = alpha;
        let tt_key = (self.board.zobrist_hash % self.num_tt_entries as u64) as usize;
        if let Some(tt_entry) = &self.transposition_table[tt_key] {
            if tt_entry.valid && tt_entry.zobrist_hash == self.board.zobrist_hash && tt_entry.depth >= depth {
                match tt_entry.flag {
                    TTFlag::Exact => return tt_entry.value,
                    TTFlag::Lowerbound => if tt_entry.value >= beta {return tt_entry.value;},
                    TTFlag::Upperbound => if tt_entry.value <= alpha {return tt_entry.value;},
                }
            }
        }

        // Check for draw types that don't involve move checking
        // Note that we don't check this at the root because we still have
        // to get and return a valid move.
        if !root {
            if evaluate::is_draw_by_insufficient_material(&self.board) || evaluate::is_draw_by_threefold_repitition(&self.board) {
                return DRAW_VALUE;
            }
        }

        // Check if we're at our search horizon
        if depth == 0 {
            return self.quiesce(alpha, beta);
        }

        // Compute ply, which will be used to store killer moves
        let ply = self.max_depth_for_search - depth + 1;

        // Generate all legal moves to search
        let my_color = if self.board.whites_turn {pieces::COLOR_WHITE} else {pieces::COLOR_BLACK};
        let mut moves = movegen::generate_all_psuedo_legal_moves(&self.board, my_color, false);

        // Score the moves
        self.score_moves(&mut moves, ply);

        // Recursively search the moves
        let mut best_move = None;
        let mut value = -INF;
        let mut pvs_active = false;
        let mut legal_move_available = false;
        for i in 0..moves.len() {

            // Grab the next highest priority move
            self.sort_move_with_priority(&mut moves, i);
            let m = &moves[i];

            // Check to make sure it's legal
            if !movegen::is_legal_move(&mut self.board, m) {
                continue;
            }
            legal_move_available = true;

            // Make the move
            self.board.make_move(m.start_square, m.end_square, None);

            // Recursively search on the new board state.
            // Note that we're going to perform a Principal Variation Search,
            // we were only fully search the first move with the hope that
            // that was the best move (which is reasonable in an iterative
            // deepening framework).  Other moves we search with a null window
            // to test whether they raise alpha (likely not).  However, if
            // they do then that means we've found a better move that the
            // previously guess PV mode, and hence have to research with the
            // full window.
            let mut score_for_move;
            if !pvs_active {
                // Search the first (and likely best) with the full window
                score_for_move = -self.negamax(depth - 1, -beta, -alpha, false);
            } else {
                // Search likely worse moves with a null window
                score_for_move = -self.negamax(depth - 1, -alpha - 1, -alpha, false);
                if alpha < score_for_move && score_for_move < beta {
                    // A better move was found; re-search with full window
                    score_for_move = -self.negamax(depth - 1, -beta, -alpha, false);
                }
            }

            // Update best move
            if score_for_move > value {
                value = score_for_move;
                best_move = Some((m.start_square as u8, m.end_square as u8));
            }

            // Unmake the move
            self.board.unmake_move();

            // Check to see if we've raised alpha, and if so we can start PVS
            if score_for_move > alpha {
                pvs_active = true;
            }

            // Check for a beta cut-off
            alpha = cmp::max(alpha, value);
            if alpha >= beta {

                // This move was strong enough to cause a beta cut-off, so
                // store it as a "killer move", which will be a high ranking
                // move to try during future move ordering calls.  If there
                // is already a killer move, shift it over so that we store
                // at most two.  Note that we don't store capture moves as
                // killer moves because they are sorted seperately.
                // See https://www.chessprogramming.org/Killer_Move
                let cur_move = Some((m.start_square as u8, m.end_square as u8));
                if m.captured_piece.is_none() && cur_move != self.primary_killers[ply as usize] {
                    self.secondary_killers[ply as usize] = self.primary_killers[ply as usize];
                    self.primary_killers[ply as usize] = cur_move;
                }

                break;
            }

        }

        // Check for checkmate and stalemate
        if !legal_move_available {
            if movegen::is_king_in_check(&self.board, my_color) {
                // The other player wins by checkmate
                return -CHECKMATE_VALUE;
            } else {
                // Stalemate
                return DRAW_VALUE;
            }
        }

        // Sanity check
        if best_move.is_none() {
            panic!("No best move found");
        }

        // Information about what already exists in the TT entry
        let mut existing_tt_entry_flag = &TTFlag::Upperbound;
        let mut existing_tt_entry_depth = 0;
        if let Some(tt_entry) = &self.transposition_table[tt_key] {
            if tt_entry.valid {
                existing_tt_entry_depth = tt_entry.depth;
                existing_tt_entry_flag = &tt_entry.flag;
            }
        }

        // Store the best move in the transposition table
        if value <= alpha_orig {

            // The best move in this subtree failed low, meaning that
            // it was not as good as an existing acceptable move.
            if matches!(existing_tt_entry_flag, TTFlag::Upperbound) && depth >= existing_tt_entry_depth {
                self.transposition_table[tt_key] = Some(TTEntry {
                    zobrist_hash: self.board.zobrist_hash,
                    depth,
                    value,
                    flag: TTFlag::Upperbound,
                    best_move: None,
                    valid: true,
                });
            }

        } else if value >= beta {

            // The best move in this subtree failed high, meaning that
            // it caused a beta cut-off.
            if matches!(existing_tt_entry_flag, TTFlag::Upperbound) || (matches!(existing_tt_entry_flag, TTFlag::Lowerbound) && depth >= existing_tt_entry_depth) {
                self.transposition_table[tt_key] = Some(TTEntry {
                    zobrist_hash: self.board.zobrist_hash,
                    depth,
                    value,
                    flag: TTFlag::Lowerbound,
                    best_move,
                    valid: true,
                });
            }

        } else {

            // The best move in this subtree is between alpha and beta,
            // meaning it is an exact value
            if matches!(existing_tt_entry_flag, TTFlag::Upperbound) || matches!(existing_tt_entry_flag, TTFlag::Lowerbound) || (matches!(existing_tt_entry_flag, TTFlag::Exact) && depth >= existing_tt_entry_depth) {
                self.transposition_table[tt_key] = Some(TTEntry {
                    zobrist_hash: self.board.zobrist_hash,
                    depth,
                    value,
                    flag: TTFlag::Exact,
                    best_move,
                    valid: true,
                });
            }

        }

        // If this is the root, store the best move
        if root {
            self.best_move_from_last_iteration = best_move;
        }
        
        // Return the score of our best move
        value

    }

    // Extract the PV line from the transposition table
    fn extract_pv_line(&mut self) -> Vec<(u8, u8, Option<usize>)> {

        let mut pv_line = Vec::new();
        let mut moves_made = 0;
        let mut zobrist_loop_detect = Vec::new();

        // Follow the PV moves in the transposition table until there are none
        // remaining at that depth.  Note that because the transposition table
        // is a hash table with limited size, it is possible that PV moves
        // were overwritten later in search.  In this case, the PV line discovered
        // may be shorter than it should be according to the depth searched.
        loop {
            let tt_key = (self.board.zobrist_hash % self.num_tt_entries as u64) as usize;
            if let Some(tt_entry) = &self.transposition_table[tt_key] {
                if tt_entry.valid && tt_entry.zobrist_hash == self.board.zobrist_hash && !zobrist_loop_detect.contains(&tt_entry.zobrist_hash) {
                    if let TTFlag::Exact = tt_entry.flag {
                        zobrist_loop_detect.push(tt_entry.zobrist_hash);
                        // TODO do we have to check to make sure this best move is legal?
                        if let Some((move_start, move_end)) = tt_entry.best_move {

                            // If pawn promotion, add the promotion piece
                            // TODO: Allow promotions to pieces other than queens.
                            let mut promotion = None;
                            if move_end >= 56 || move_end <= 7 {
                                if let Some((_,p)) = self.board.get_color_and_piece_on_square(move_start as usize) {
                                    if p == pieces::PAWN {
                                        promotion = Some(pieces::QUEEN);
                                    }
                                }
                            }
                            
                            pv_line.push((move_start, move_end, promotion));
                            self.board.make_move(move_start as usize, move_end as usize, None);
                            moves_made += 1;
                        }
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        for _i in 0..moves_made {
            self.board.unmake_move();
        }
        pv_line
    }

}

// =====================================
//             UNIT TESTS
// =====================================

#[cfg(test)]
mod tests {
    
    use std::sync::mpsc::{Sender, Receiver};
    use std::sync::mpsc;
    use crate::chess_board::ChessBoard;
    use super::*;

    // Test SEE
    #[test]
    fn test_see_capture() {
        // Force a set of bitboards to look like this
        // ........
        // ...q....
        // ........
        // ...p.r..
        // ........
        // .Q......
        // B.......
        // ........
        let (_, rx): (Sender<String>, Receiver<String>) = mpsc::channel();
        let mut board = ChessBoard::new();
        board.bb_pieces[pieces::COLOR_WHITE][pieces::QUEEN] = bitboard::to_bb(17);
        board.bb_pieces[pieces::COLOR_WHITE][pieces::BISHOP] = bitboard::to_bb(8);
        board.bb_pieces[pieces::COLOR_BLACK][pieces::QUEEN] = bitboard::to_bb(51);
        board.bb_pieces[pieces::COLOR_BLACK][pieces::PAWN] = bitboard::to_bb(35);
        board.bb_pieces[pieces::COLOR_BLACK][pieces::ROOK] = bitboard::to_bb(37);
        board.bb_occupied_squares = 0;
        for color in 0..2 {
            for piece in 0..6 {
                board.bb_occupied_squares ^= board.bb_pieces[color][piece];
            }
        }
        let m = movegen::ChessMove {
            start_square: 17,
            end_square: 35,
            piece: pieces::QUEEN,
            captured_piece: Some(pieces::PAWN),
            priority: 0,
            is_en_passant: false,
        };
        let searcher = SearchEngine {
            rx_channel: rx,
            board,
            num_tt_entries: 0,
            transposition_table: Vec::new(),
            primary_killers: [None; 100],
            secondary_killers: [None; 100],
            max_depth_for_search: 0,
            best_move_from_last_iteration: None,
            moves_analyzed: 0,
            time_max_for_move: 0,
            move_start_time: time::Instant::now(),
            halt_search: false,
            halt_check_countdown: CHECK_HALT_CONDITION_INTERVAL,
        };
        let see_value = searcher.see_capture_eval(&m);
        assert_eq!(see_value, -600);

        // Force a set of bitboards to look like this
        // ...Q....
        // ...q....
        // ........
        // ...p.r..
        // ........
        // .B......
        // Q.......
        // ........
        let (_, rx): (Sender<String>, Receiver<String>) = mpsc::channel();
        let mut board = ChessBoard::new();
        board.bb_pieces[pieces::COLOR_WHITE][pieces::QUEEN] = bitboard::to_bb(8) | bitboard::to_bb(59);
        board.bb_pieces[pieces::COLOR_WHITE][pieces::BISHOP] = bitboard::to_bb(17);
        board.bb_pieces[pieces::COLOR_BLACK][pieces::QUEEN] = bitboard::to_bb(51);
        board.bb_pieces[pieces::COLOR_BLACK][pieces::PAWN] = bitboard::to_bb(35);
        board.bb_pieces[pieces::COLOR_BLACK][pieces::ROOK] = bitboard::to_bb(37);
        board.bb_occupied_squares = 0;
        for color in 0..2 {
            for piece in 0..6 {
                board.bb_occupied_squares ^= board.bb_pieces[color][piece];
            }
        }
        let m = movegen::ChessMove {
            start_square: 17,
            end_square: 35,
            piece: pieces::BISHOP,
            captured_piece: Some(pieces::PAWN),
            priority: 0,
            is_en_passant: false,
        };
        let searcher = SearchEngine {
            rx_channel: rx,
            board,
            num_tt_entries: 0,
            transposition_table: Vec::new(),
            primary_killers: [None; 100],
            secondary_killers: [None; 100],
            max_depth_for_search: 0,
            best_move_from_last_iteration: None,
            moves_analyzed: 0,
            time_max_for_move: 0,
            move_start_time: time::Instant::now(),
            halt_search: false,
            halt_check_countdown: CHECK_HALT_CONDITION_INTERVAL,
        };
        let see_value = searcher.see_capture_eval(&m);
        assert_eq!(see_value, 100);
    }

}