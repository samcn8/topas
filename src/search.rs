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
//! depths, storing the principal variable (PV) -- the best line
//! computed so far -- as the best candidate move for the next depth.

use std::time;
use std::cmp;
use crate::evaluate;
use crate::chess_board;
use crate::movegen;
use crate::pieces;

// Default number of TT entries
const DEFAULT_NUM_TT_ELEMENTS: usize = 1000000;

// Scores for terminal states and infinity
const CHECKMATE_VALUE: i32 = 50000;
const DRAW_VALUE: i32 = 0;
const INF: i32 = 1000000;

// When prioritizing moves, a bonus may be assigned to a move.
// Principal variable (PV) moves are the most valuable, and are
// usually discovered on the previous iterative deepening loop.
// Moves that lead to a beta cutoff are also very valuable as they
// can signficantly decrease the search space.  Promotions and
// captures are valuable, followed by quiet moves.
const PV_MOVE_PRIORITY_BONUS: i32 = 400;
const CUTOFF_PRIORITY_BONUS: i32 = 300;
const PROMOTION_PRIORITY_BONUS: i32 = 200;
const CAPTURE_PRIORITY_BONUS: i32 = 100;

// TT Flag corresponding to a value
enum TTFlag {

    // An exact value is one that falls between alpha and beta and
    // represents a PV move
    Exact,

    // A lower bound value is one that failed high and caused a
    // beta-cutoff -- the move was too good
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
    // and hits 0 at the ends of the standard search.
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

// Information about the top move discovered from a search depth
#[derive(Debug)]
pub struct BestMoveInformation {

    // True if we are still searching.  This means that we've
    // completed an iterative deepening loop, but we still have
    // more to go.  If this is true, then this is for informational
    // purposes only.  If this is false, it is the final best move.
    pub still_searching: bool,

    // The best move
    // Represents (start square, end square)
    pub best_move_from_last_iteration: Option<(u8, u8)>,

    // The value / score from the engine's perspective assuming
    // the best move is played
    pub value: i32,

    // The max depth that ended up being searched in the iterative
    // deepening loop
    pub depth_searched: u8,

    // The total moves that ended up being searched
    pub moves_analyzed: i32,

    // Time in milliseconds that it took to find this move
    pub duration_of_search: u128,

    // The PV line computed
    pub pv_line: Vec<(u8, u8)>,

}

// The main engine
pub struct SearchEngine {

    // The game board
    board: chess_board::ChessBoard,

    // The transposition table size in entries.  Each entry
    // is 24B so the total size of the TT is: 24B * num_tt_entries.
    num_tt_entries: usize,

    // The transposition table.
    transposition_table: Vec<Option<TTEntry>>,
    
    // The stored best move from the last iteration
    // Represents (start square, end square)
    best_move_from_last_iteration: Option<(u8, u8)>,

    // Total moves analyzed in current search
    moves_analyzed: i32,

}

impl SearchEngine {

    // Construct a new SearchEngine
    pub fn new() -> SearchEngine {
        SearchEngine {
            board: chess_board::ChessBoard::new(),
            num_tt_entries: DEFAULT_NUM_TT_ELEMENTS,
            transposition_table: Vec::new(),
            best_move_from_last_iteration: None,
            moves_analyzed: 0,
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
    // The move string is in long algebraic notation without piece names,
    // with spaces between each move, as dictated by the UCI protocol.
    // move.  For instance, "e2e4 b8c6".  Promotion looks like "f7f8q".
    // TODO: Handle FEN string input if not starting at the start position.
    fn set_board_state(&mut self, move_str: &str) {

        // Get the list of moves passed in
        let moves = movegen::convert_moves_str_into_list(move_str);

        // Start the board at a given starting position
        self.board.new_game();

        // Play out the provided moves
        for (start_square, end_square) in moves {
            self.board.make_move(start_square, end_square);
        }

        // Reset other state
        self.best_move_from_last_iteration = None;
        self.moves_analyzed = 0;

    }

    // This returns a priority bonus for move ordering if the move is
    // a PV move or causes a beta cutoff.  This is determined via lookup
    // in the transposition table.
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

    // This sorts moves, in place, with the highest priority moves first.
    // Priority from high to low is: (1) PV moves, (2) moves that cause
    // a beta cut-off, (3) captures, sorted by MVV-LVA, and (4) quiet moves.
    fn sort_moves(&self, moves: &mut Vec<movegen::ChessMove>) {

        // Assign a priority to all moves
        for m in moves.iter_mut() {

            // Check the transposition table for PV and cut-off moves
            let mut priority = self.get_move_priority_bonus(m.start_square, m.end_square);

            // Check for promotions
            if m.piece == pieces::PAWN && (m.end_square / 8 == 0 || m.end_square / 8 == 7) {
                priority += PROMOTION_PRIORITY_BONUS;
            }

            // Check for captures, and prioritize based on MVV-LVA
            if let Some(cap) = m.captured_piece {
                priority += CAPTURE_PRIORITY_BONUS + pieces::MVV_LVA[cap][m.piece];
            }

            // Set priority
            m.priority = priority;

        }

        // Sort moves by priority
        moves.sort_unstable_by(|a, b| b.priority.cmp(&a.priority));
    }

    // This is an implementation of the quiescence search, which allows
    // the engine to keep searching "non-quiet" (such as capture) moves
    // beyond the search horizon.  This is done to mitigate the horizon
    // effect, which may cause a bad decision to be made right at the edge
    // of the search horizon.
    // See https://www.chessprogramming.org/Quiescence_Search
    fn quiesce(&mut self, mut alpha: i32, beta: i32) -> i32 {
        
        // This is our stand pat score, which is the current score
        // of the board without additional moves.
        let stand_pat = evaluate::static_evaluation(&self.board);
        
        // Check for a beta cut-off
        if stand_pat >= beta {
            return beta;
        }

        // Delta pruning
        // See https://www.chessprogramming.org/Delta_Pruning
        if stand_pat < alpha - pieces::PIECE_VALUES[pieces::QUEEN] {
            return alpha;
        }

        // Generate all legal moves.  Note that we will only search
        // non-quiet moves.
        let my_color = if self.board.whites_turn {pieces::COLOR_WHITE} else {pieces::COLOR_BLACK};
        let mut moves = movegen::generate_all_psuedo_legal_moves(&self.board, my_color);
        movegen::retain_only_legal_moves(&mut self.board, &mut moves);
        self.sort_moves(&mut moves);

        // Check for checkmate and stalemate
        if moves.len() == 0 {
            if movegen::is_king_in_check(&self.board, my_color) {
                // The other player wins by checkmate
                return -CHECKMATE_VALUE;
            } else {
                // Stalemate
                return DRAW_VALUE;
            }
        }

        // Recursively search the non-quiet moves
        for m in moves.iter() {

            // Filter out non-captures
            if m.captured_piece.is_none() {
                continue;
            }

            // Make the move
            self.board.make_move(m.start_square, m.end_square);

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

        // Return alpha, which is the best we can do without failing high
        alpha

    }

    // This is an implementation of the minimax algorithm with alpha-beta
    // pruning and is the core of the engine's search routine.  This uses
    // transposition table lookups to enhance performance.
    // See https://en.wikipedia.org/wiki/Negamax
    fn negamax(&mut self, depth: u8, mut alpha: i32, mut beta: i32, root: bool) -> i32 {
        
        // Update moves analyzed count
        self.moves_analyzed += 1;

        // Check transposition tables for any cached values
        let alpha_orig = alpha;
        let tt_key = (self.board.zobrist_hash % self.num_tt_entries as u64) as usize;
        if let Some(tt_entry) = &self.transposition_table[tt_key] {
            if tt_entry.valid && tt_entry.zobrist_hash == self.board.zobrist_hash && tt_entry.depth >= depth {
                match tt_entry.flag {
                    TTFlag::Exact => return tt_entry.value,
                    TTFlag::Lowerbound => alpha = cmp::max(alpha, tt_entry.value),
                    TTFlag::Upperbound => beta = cmp::min(beta, tt_entry.value),
                }
                if alpha >= beta {
                    return tt_entry.value;
                }
            }
        }

        // Check for draw types that don't involve move checking
        if evaluate::is_draw_by_insufficient_material(&self.board) || evaluate::is_draw_by_threefold_repitition(&self.board) {
            return DRAW_VALUE;
        }

        // Check if we're at our search horizon
        if depth == 0 {
            return self.quiesce(alpha, beta);
        }

        // Generate all legal moves to search
        let my_color = if self.board.whites_turn {pieces::COLOR_WHITE} else {pieces::COLOR_BLACK};
        let mut moves = movegen::generate_all_psuedo_legal_moves(&self.board, my_color);
        movegen::retain_only_legal_moves(&mut self.board, &mut moves);
        self.sort_moves(&mut moves);

        // Check for checkmate and stalemate
        if moves.len() == 0 {
            if movegen::is_king_in_check(&self.board, my_color) {
                // The other player wins by checkmate
                return -CHECKMATE_VALUE;
            } else {
                // Stalemate
                return DRAW_VALUE;
            }
        }

        // Recursively search the moves
        let mut best_move = None;
        let mut value = -INF;
        for m in moves.iter() {

            // Make the move
            self.board.make_move(m.start_square, m.end_square);

            // Recursively search on the new board state
            let score_for_move = -self.negamax(depth - 1, -beta, -alpha, false);

            // Update best move
            if score_for_move > value {
                value = score_for_move;
                best_move = Some((m.start_square as u8, m.end_square as u8));
            }

            // Unmake the move
            self.board.unmake_move();

            // Check for a beta cut-off
            alpha = cmp::max(alpha, value);
            if alpha >= beta {
                break;
            }

        }

        // Sanity check
        if best_move.is_none() {
            panic!("No best move found");
        }

        // Store the best move in the transposition table
        if value <= alpha_orig {

            // The best move in this subtree failed low, meaning that
            // it was not as good as an existing acceptable move.
            self.transposition_table[tt_key] = Some(TTEntry {
                zobrist_hash: self.board.zobrist_hash,
                depth,
                value,
                flag: TTFlag::Upperbound,
                best_move: None,
                valid: true,
            });

        } else if value >= beta {

            // The best move in this subtree failed high, meaning that
            // it was too good and caused a beta cut-off.
            self.transposition_table[tt_key] = Some(TTEntry {
                zobrist_hash: self.board.zobrist_hash,
                depth,
                value,
                flag: TTFlag::Lowerbound,
                best_move,
                valid: true,
            });

        } else {

            // The best move in this subtree is between alpha and beta,
            // meaning it is a PV move.
            self.transposition_table[tt_key] = Some(TTEntry {
                zobrist_hash: self.board.zobrist_hash,
                depth,
                value,
                flag: TTFlag::Exact,
                best_move,
                valid: true,
            });

        }

        // If this is the root, store the best move
        if root {
            self.best_move_from_last_iteration = best_move;
        }
        
        // Return the score of our best move (which is also now alpha)
        value

    }

    // This returns the engine's top move given a maximum search depth.
    // This uses self.board as the current state of the board to search from.
    // This uses an iterative deepening search.  The PV move found in the
    // previous iteration is the first searched node in the next iteration.
    pub fn find_best_move(&mut self, max_depth: u8) -> Option<BestMoveInformation> {

        // Santity check
        if max_depth == 0 {
            panic!("Invalid search depth of 0");
        }

        // Information about the last iteration
        let mut last_iteration_info: Option<BestMoveInformation> = None;

        // Start of iterative deepening loop
        let mut value: i32;
        for depth in 1..(max_depth+1) {
            
            // Timing
            let start_time_iteration = time::Instant::now();

            // Find the best move using negamax
            value = self.negamax(depth, -INF, INF, true);

            let duration_iteration = start_time_iteration.elapsed();

            // Create a record for it, and give it back to the caller
            last_iteration_info = Some(BestMoveInformation {
                still_searching: if depth == max_depth {false} else {true},
                best_move_from_last_iteration: self.best_move_from_last_iteration,
                value,
                moves_analyzed: self.moves_analyzed,
                depth_searched: depth,
                duration_of_search: duration_iteration.as_millis(),
                pv_line: Vec::new(),
            });

            // TODO - send this to caller
            println!("{:?}", last_iteration_info);

            // Reset some state for next iteration
            self.best_move_from_last_iteration = None;
            self.moves_analyzed = 0;
        }

        // Clear out the transposition tables
        self.transposition_table.clear();
        self.transposition_table.resize_with(self.num_tt_entries, ||-> Option<TTEntry> {None});

        // Provide the best move to the caller
        last_iteration_info

    }

}