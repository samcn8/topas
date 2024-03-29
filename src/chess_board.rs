//! This module contains the entire state of the game board and past moves.
//! This does not generate or validate any moves; it assumes all moves
//! passed through "make_move" have already been validated.
//!
//! This uses the "Least Significant File Mapping" representation:
//! 56 57 58 59 60 61 62 63
//! 48 49 50 51 52 53 54 55
//! 40 41 42 43 44 45 46 47
//! 32 33 34 35 36 37 38 39
//! 24 25 26 27 28 29 30 31
//! 16 17 18 19 20 21 22 23
//!  8  9 10 11 12 13 14 15
//!  0  1  2  3  4  5  6  7

use crate::bitboard;
use crate::zobrist;
use crate::pieces;

// FEN string for the starting position
pub const STARTFEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

// Convert a file in 0-7 and rank in 0-7 to a square ID
pub fn file_rank_to_square(file: usize, rank: usize) -> usize {
    rank * 8 + file
}

#[derive(Debug)]
struct MoveRecord {

    // Starting square from 0 to 63
    start_square: usize,

    // Ending square from 0 to 63
    end_square: usize,

    // Piece that was moved
    piece: usize,

    // Captured piece, if applicable
    captured_piece: Option<usize>,

    // Is this move an en passant capture?
    is_en_passant: bool,

    // If a promotion occured, this is the promotion square
    promotion_square: Option<usize>,

    // If a promotion occured, this is the new piece
    promotion_piece: Option<usize>,

    // Game state from before the move, for unmake_move purposes
    prior_white_ks_castling_rights: bool,
    prior_white_qs_castling_rights: bool,
    prior_black_ks_castling_rights: bool,
    prior_black_qs_castling_rights: bool,
    prior_en_passant_rights: Option<usize>,
}

pub struct ChessBoard {

    // Bitboards representing occupied squares for a [color][piece]
    // for a total of 12
    pub bb_pieces: Vec<Vec<u64>>,

    // Bitboards representing all occupied squares for a side (white
    // and black)
    pub bb_side: Vec<u64>,

    // Bitboard representing all occupied squares for the entire board
    pub bb_occupied_squares: u64,

    // Bitboard representing all empty squares for the entire board
    pub bb_empty_squares: u64,

    // List of all moves from the start of the game
    move_history: Vec<MoveRecord>,

    // Zobrist hash cooresponding to the board after each move in the history
    pub zobrist_history: Vec<u64>,

    // True if white's turn, false if black's turn
    pub whites_turn: bool,

    // Castling rights (whether a castle is still possible or not)
    // ks = king side, qs = queen side
    pub white_ks_castling_rights: bool,
    pub white_qs_castling_rights: bool,
    pub black_ks_castling_rights: bool,
    pub black_qs_castling_rights: bool,

    // If not None, this indicates the active en passant square.
    // This is the square the opposing pawn just moved through on a two step
    // move, if the current player can capture en passant to that square.
    pub en_passant_rights: Option<usize>,

    // Zobrist hash of the current board state
    zobrist_hasher: zobrist::ZobristHasher,
    pub zobrist_hash: u64,
}

impl ChessBoard {

    // Construct a new ChessBoard
    pub fn new() -> ChessBoard {
        ChessBoard {
            bb_pieces: vec![vec![0; 6]; 2],
            bb_side: vec![0; 2],
            bb_occupied_squares: 0,
            bb_empty_squares: 0,
            move_history: Vec::new(),          
            zobrist_history: Vec::new(),
            whites_turn: true,
            white_ks_castling_rights: true,
            white_qs_castling_rights: true,
            black_ks_castling_rights: true,
            black_qs_castling_rights: true,
            en_passant_rights: None,
            zobrist_hasher: zobrist::ZobristHasher::new(),
            zobrist_hash: 0,
        }
    }

    // Set / reset the game from the starting position
    pub fn new_game(&mut self) {
        self.new_game_from_fen(STARTFEN);
    }

    // Set / reset the game state to the point listed in the
    // FEN string (see https://en.wikipedia.org/wiki/Forsyth–Edwards_Notation)
    pub fn new_game_from_fen(&mut self, fen_str: &str) {

        // Get the 6 components of the FEN string
        let tokens: Vec<&str> = fen_str.split_whitespace().collect();
        if tokens.len() != 6 {
            panic!("Invalid FEN string: {}", fen_str);
        }

        // Component 1: Piece placement
        let piece_tokens: Vec<&str> = tokens[0].split('/').collect();
        if piece_tokens.len() != 8 {
            panic!("Invalid FEN string: {}", fen_str);
        }
        for color in 0..2 {
            for piece in 0..6 {
                self.bb_pieces[color][piece] = 0;
            }
        }
        for (i, file_str) in piece_tokens.iter().enumerate() {
            let mut square = (7-i)*8;
            for c in file_str.chars() {
                if c.is_digit(10) {
                    if let Some(d) = c.to_digit(10) {
                        square += d as usize;
                    } else {
                        panic!("Invalid FEN string: {}", fen_str);
                    }
                } else if c.is_lowercase() {
                    let piece = pieces::PIECE_ID_TO_CHAR[pieces::COLOR_BLACK].iter().position(|&r| r == c).unwrap();
                    self.bb_pieces[pieces::COLOR_BLACK][piece] |= bitboard::to_bb(square);
                    square += 1;
                } else if c.is_uppercase() {
                    let piece = pieces::PIECE_ID_TO_CHAR[pieces::COLOR_WHITE].iter().position(|&r| r == c).unwrap();
                    self.bb_pieces[pieces::COLOR_WHITE][piece] |= bitboard::to_bb(square);
                    square += 1;
                } else {
                    panic!("Invalid FEN string: {}", fen_str);
                }
            }
        }

        // Reset side and occupied bitboards
        for c in 0..2 {
            self.bb_side[c] = 0;
            for p in self.bb_pieces[c].iter() {
                self.bb_side[c] |= p;
            }
        }
        self.bb_occupied_squares = self.bb_side[pieces::COLOR_WHITE] | self.bb_side[pieces::COLOR_BLACK];
        self.bb_empty_squares = !self.bb_occupied_squares;

        // Component 2: Turn
        if tokens[1] == "w" {
            self.whites_turn = true;
        } else if tokens[1] == "b" {
            self.whites_turn = false;
        } else {
            panic!("Invalid FEN string: {}", fen_str);
        }

        // Component 3: Castling rights
        self.white_ks_castling_rights = false;
        self.white_qs_castling_rights = false;
        self.black_ks_castling_rights = false;
        self.black_qs_castling_rights = false;
        if tokens[2].contains("K") {
            self.white_ks_castling_rights = true;
        }
        if tokens[2].contains("Q") {
            self.white_qs_castling_rights = true;
        }
        if tokens[2].contains("k") {
            self.black_ks_castling_rights = true;
        }
        if tokens[2].contains("q") {
            self.black_qs_castling_rights = true;
        }

        // Component 4: En passant target square
        // TODO - we represent en passant rights only if a legal en passant
        // capture is possible.  However, the older (and more common) FEN
        // notation adds the en passant target square if a pawn moves two
        // spaces, regardless if an en passant capture is possible.  We
        // have to reconcile this before we can use this information.
        self.en_passant_rights = None;

        // Component 5: Halfmove clock
        // TODO

        // Component 6: Fullmove number
        // TODO

        // Reset the rest of the state
        self.move_history.clear();
        self.zobrist_history.clear();

        // Reset the Zobrist hash
        self.zobrist_hash = self.zobrist_hasher.full_hash(self);

    }

    // Perform a move and update the game state accordingly.  This assumes
    // that the move has already been verified to be legal.  This function
    // will be called a large number of times during a search, and so the
    // performance of this function is critical to the speed of the engine.
    // NOTE: if None is passed in as the promotion piece, and a promotion
    // is required by the move, then this will assume a queen promotion.
    pub fn make_move(&mut self, start_square: usize, end_square: usize, promotion_piece: Option<usize>) {

        // Get rank (0-7) and file (0-7) for important squares
        let start_rank = start_square / 8;
        let end_rank = end_square / 8;
        let end_file = end_square % 8;

        // Get colors
        let my_color = if self.whites_turn {pieces::COLOR_WHITE} else {pieces::COLOR_BLACK};
        let opp_color = if self.whites_turn {pieces::COLOR_BLACK} else {pieces::COLOR_WHITE};

        // Get piece
        let piece = match self.get_color_and_piece_on_square(start_square) {
            Some((_,p)) => p,
            None => panic!("No piece on starting square passed to make_move"),
        };

        // Get capture if available (note en passant is handled later)
        let mut captured_piece: Option<usize> = None;
        if let Some((_, p)) = self.get_color_and_piece_on_square(end_square) {
            captured_piece = Some(p);
        }

        // Check whether this is an en passant capture. While we're
        // at it, check if this is a promotion (for undo move purposes).
        let mut is_en_passant = false;
        let mut promotion_square = None;
        if piece == pieces::PAWN {
            if let Some(e) = self.en_passant_rights {
                if e == end_square {
                    is_en_passant = true;
                    captured_piece = Some(pieces::PAWN)
                }
            }
            if end_rank == 0 || end_rank == 7 {
                // The only way for a pawn (of any color) to end up on
                // rank 0 or 7 is if they are promoting.
                promotion_square = Some(end_square);
            }
        }

        // Create and store a move record for this move
        let move_record = MoveRecord {
            start_square,
            end_square,
            piece,
            captured_piece,
            is_en_passant,
            promotion_square,
            promotion_piece,
            prior_white_ks_castling_rights: self.white_ks_castling_rights,
            prior_white_qs_castling_rights: self.white_qs_castling_rights,
            prior_black_ks_castling_rights: self.black_ks_castling_rights,
            prior_black_qs_castling_rights: self.black_qs_castling_rights,
            prior_en_passant_rights: self.en_passant_rights,
        };
        self.move_history.push(move_record);

        // Check if we have to give our opponent en passant rights
        let mut give_en_passant_rights = false;
        if piece == pieces::PAWN && (start_rank == 1 && end_rank == 3 || start_rank == 6 && end_rank == 4) {

            // This is a double-square pawn push
            let opponent_pawns = bitboard::occupied_squares(self.bb_pieces[opp_color][pieces::PAWN]);
            if end_file > 0 && opponent_pawns.contains(&file_rank_to_square(end_file-1, end_rank)) || 
                end_file < 7 && opponent_pawns.contains(&file_rank_to_square(end_file+1, end_rank)) {
                give_en_passant_rights = true;
                // Undo old en passant rights if needed
                if let Some(e) = self.en_passant_rights {
                    self.zobrist_hash ^= self.zobrist_hasher.hash_en_passant[e % 8];
                }
                // Update new en passant rights
                self.zobrist_hash ^= self.zobrist_hasher.hash_en_passant[end_file];
                if self.whites_turn {
                    self.en_passant_rights = Some(file_rank_to_square(end_file, end_rank-1));
                } else {
                    self.en_passant_rights = Some(file_rank_to_square(end_file, end_rank+1));
                }
            }

        }
        if !give_en_passant_rights {
            // Undo old en passant rights, if needed
            if let Some(e) = self.en_passant_rights {
                self.zobrist_hash ^= self.zobrist_hasher.hash_en_passant[e % 8];
            }
            self.en_passant_rights = None;
        }

        // Bitboards representing to and from squares
        let from_bb = bitboard::to_bb(start_square);
        let to_bb = bitboard::to_bb(end_square);
        let from_to_bb = from_bb ^ to_bb;

        // Move source to dest
        self.bb_pieces[my_color][piece] ^= from_to_bb;
        self.bb_side[my_color] ^= from_to_bb;
        
        // Hash - place the source on dest, and revert the source square
        self.zobrist_hash ^= self.zobrist_hasher.hash_piece[end_square][my_color][piece];
        self.zobrist_hash ^= self.zobrist_hasher.hash_piece[start_square][my_color][piece];

        // Handle potential captures
        if let Some(cp) = captured_piece {
            // A capture occured
            if is_en_passant {
                // Remove captured pawn from board
                let captured_pawn_square: usize = if self.whites_turn {file_rank_to_square(end_file, end_rank-1)} else {file_rank_to_square(end_file, end_rank+1)};
                let captured_pawn_square_bb = bitboard::to_bb(captured_pawn_square);
                self.bb_pieces[opp_color][cp] ^= captured_pawn_square_bb;
                self.bb_side[opp_color] ^= captured_pawn_square_bb;
                self.bb_occupied_squares ^= from_to_bb;
                self.bb_empty_squares ^= from_to_bb;
                self.bb_occupied_squares ^= captured_pawn_square_bb;
                self.bb_empty_squares ^= captured_pawn_square_bb;
                self.zobrist_hash ^= self.zobrist_hasher.hash_piece[captured_pawn_square][opp_color][cp];
            } else {
                // Remove captured piece from board
                self.bb_pieces[opp_color][cp] ^= to_bb;
                self.bb_side[opp_color] ^= to_bb;
                self.bb_occupied_squares ^= from_bb;
                self.bb_empty_squares ^= from_bb;
                self.zobrist_hash ^= self.zobrist_hasher.hash_piece[end_square][opp_color][cp];
            }
        } else {
            // There was no capture; this is a "quiet" move
            self.bb_occupied_squares ^= from_to_bb;
            self.bb_empty_squares ^= from_to_bb;
        }

        // Handle promotion.  By the time we get here the pawn bitboard
        // will have been updated already, with the pawn on the promotion
        // square.  Therefore, we don't have to change the bb_side or
        // bb_occupied_squares bitboards.
        if promotion_square.is_some() {
            // TODO: Be more explicit about queen promotions; this currently
            // will assume the caller wants a queen promotion if not otherwise
            // specified.
            let pp = if let Some(s) = promotion_piece {s} else {pieces::QUEEN};
            self.bb_pieces[my_color][pieces::PAWN] ^= to_bb;
            self.bb_pieces[my_color][pp] ^= to_bb;
            self.zobrist_hash ^= self.zobrist_hasher.hash_piece[end_square][my_color][pieces::PAWN];
            self.zobrist_hash ^= self.zobrist_hasher.hash_piece[end_square][my_color][pp];
        } 

        // If this was a castling move, we now have to take care to move
        // the rook around the king.
        // Square 4 -> 6 is white kingside castling.  Rook 7 -> 5.
        // Square 4 -> 2 is white queenside castling.  Rook 0 -> 3.
        // Square 60 -> 62 is black kingside castling.  Rook 63 -> 61.
        // Square 60 -> 58 is black queenside castling.  Rook 56 -> 59.
        if piece == pieces::KING {
            if start_square == 4 && end_square == 6 {
                self.bb_pieces[my_color][pieces::ROOK] ^= bitboard::BB_WKS_CASTLING_ROOKS_FROM_TO;
                self.bb_side[my_color] ^= bitboard::BB_WKS_CASTLING_ROOKS_FROM_TO;
                self.bb_occupied_squares ^= bitboard::BB_WKS_CASTLING_ROOKS_FROM_TO;
                self.bb_empty_squares ^= bitboard::BB_WKS_CASTLING_ROOKS_FROM_TO;
                // Hash - apply rook to new square and revert it from old square
                self.zobrist_hash ^= self.zobrist_hasher.hash_piece[7][my_color][pieces::ROOK];
                self.zobrist_hash ^= self.zobrist_hasher.hash_piece[5][my_color][pieces::ROOK];
            } else if start_square == 4 && end_square == 2 {
                self.bb_pieces[my_color][pieces::ROOK] ^= bitboard::BB_WQS_CASTLING_ROOKS_FROM_TO;
                self.bb_side[my_color] ^= bitboard::BB_WQS_CASTLING_ROOKS_FROM_TO;
                self.bb_occupied_squares ^= bitboard::BB_WQS_CASTLING_ROOKS_FROM_TO;
                self.bb_empty_squares ^= bitboard::BB_WQS_CASTLING_ROOKS_FROM_TO;
                // Hash - apply rook to new square and revert it from old square
                self.zobrist_hash ^= self.zobrist_hasher.hash_piece[0][my_color][pieces::ROOK];
                self.zobrist_hash ^= self.zobrist_hasher.hash_piece[3][my_color][pieces::ROOK];
            } else if start_square == 60 && end_square == 62 {
                self.bb_pieces[my_color][pieces::ROOK] ^= bitboard::BB_BKS_CASTLING_ROOKS_FROM_TO;
                self.bb_side[my_color] ^= bitboard::BB_BKS_CASTLING_ROOKS_FROM_TO;
                self.bb_occupied_squares ^= bitboard::BB_BKS_CASTLING_ROOKS_FROM_TO;
                self.bb_empty_squares ^= bitboard::BB_BKS_CASTLING_ROOKS_FROM_TO;
                // Hash - apply rook to new square and revert it from old square
                self.zobrist_hash ^= self.zobrist_hasher.hash_piece[63][my_color][pieces::ROOK];
                self.zobrist_hash ^= self.zobrist_hasher.hash_piece[61][my_color][pieces::ROOK];
            } else if start_square == 60 && end_square == 58 {
                self.bb_pieces[my_color][pieces::ROOK] ^= bitboard::BB_BQS_CASTLING_ROOKS_FROM_TO;
                self.bb_side[my_color] ^= bitboard::BB_BQS_CASTLING_ROOKS_FROM_TO;
                self.bb_occupied_squares ^= bitboard::BB_BQS_CASTLING_ROOKS_FROM_TO;
                self.bb_empty_squares ^= bitboard::BB_BQS_CASTLING_ROOKS_FROM_TO;
                // Hash - apply rook to new square and revert it from old square
                self.zobrist_hash ^= self.zobrist_hasher.hash_piece[56][my_color][pieces::ROOK];
                self.zobrist_hash ^= self.zobrist_hasher.hash_piece[59][my_color][pieces::ROOK];
            }
        }

        // Update castling rights based on a king being moved
        let mut wks = true;
        let mut wqs = true;
        let mut bks = true;
        let mut bqs = true;
        if self.whites_turn && piece == pieces::KING && start_square == 4 {
            wks = false;
            wqs = false;
        } else if !self.whites_turn && piece == pieces::KING && start_square == 60 {
            bks = false;
            bqs = false;
        }

        // Update castling rights based on a rook being moved
        if self.whites_turn && piece == pieces::ROOK && start_square == 7 {
            wks = false;
        } else if self.whites_turn && piece == pieces::ROOK && start_square == 0 {
            wqs = false;
        } else if !self.whites_turn && piece == pieces::ROOK && start_square == 63 {
            bks = false;
        } else if !self.whites_turn && piece == pieces::ROOK && start_square == 56 {
            bqs = false;
        }

        // Update castling rights based on a rook being captured
        if let Some(cp) = captured_piece {
            if cp == pieces::ROOK {
                if self.whites_turn && end_square == 63 {
                    bks = false;
                } else if self.whites_turn && end_square == 56 {
                    bqs = false;
                } else if !self.whites_turn && end_square == 7 {
                    wks = false;
                } else if !self.whites_turn && end_square == 0 {
                    wqs = false;
                }
            }
        }

        // Remove castling rights
        if self.white_ks_castling_rights && !wks {
            self.white_ks_castling_rights = false;
            self.zobrist_hash ^= self.zobrist_hasher.hash_white_ks_castling_rights;
        }
        if self.white_qs_castling_rights && !wqs {
            self.white_qs_castling_rights = false;
            self.zobrist_hash ^= self.zobrist_hasher.hash_white_qs_castling_rights;
        }
        if self.black_ks_castling_rights && !bks {
            self.black_ks_castling_rights = false;
            self.zobrist_hash ^= self.zobrist_hasher.hash_black_ks_castling_rights;
        }
        if self.black_qs_castling_rights && !bqs {
            self.black_qs_castling_rights = false;
            self.zobrist_hash ^= self.zobrist_hasher.hash_black_qs_castling_rights;
        }

        // Change side
        self.whites_turn = !self.whites_turn;
        self.zobrist_hash ^= self.zobrist_hasher.hash_blacks_turn;

        // Store Zobrist hash in history
        self.zobrist_history.push(self.zobrist_hash);

    }

    // Undo the last move.  This restores all state to the state prior
    // to the last move made - the Zobrist hashes should be the same.
    // This function will be called a large number of times during a search,
    // and so the performance of this function is critical to the speed of
    // the engine.
    pub fn unmake_move(&mut self) {

        // Remove Zobrist hash from history
        self.zobrist_history.pop();

        // Get the last move from history
        let last_move = if let Some(e) = self.move_history.pop() {
            e
        } else {
            panic!("Trying to unmake move with empty move history");
        };

        // Change side
        self.zobrist_hash ^= self.zobrist_hasher.hash_blacks_turn;
        self.whites_turn = !self.whites_turn;

        // Get rank (0-7) for important squares
        let end_rank = last_move.end_square / 8;
        let end_file = last_move.end_square % 8;

        // Get colors
        let my_color = if self.whites_turn {pieces::COLOR_WHITE} else {pieces::COLOR_BLACK};
        let opp_color = if self.whites_turn {pieces::COLOR_BLACK} else {pieces::COLOR_WHITE};

        // Restore en passant rights if they changed
        if last_move.prior_en_passant_rights != self.en_passant_rights {
            // Undo old en passant rights, if needed
            if let Some(e) = self.en_passant_rights {
                self.zobrist_hash ^= self.zobrist_hasher.hash_en_passant[e % 8];
            }
            // Set en passant rights
            if let Some(e) = last_move.prior_en_passant_rights {
                self.zobrist_hash ^= self.zobrist_hasher.hash_en_passant[e % 8]
            }
            self.en_passant_rights = last_move.prior_en_passant_rights;
        }

        // Restore castling rights if they changed
        let wks = last_move.prior_white_ks_castling_rights;
        let wqs = last_move.prior_white_qs_castling_rights;
        let bks = last_move.prior_black_ks_castling_rights;
        let bqs = last_move.prior_black_qs_castling_rights;
        if wks != self.white_ks_castling_rights {
            self.white_ks_castling_rights = wks;
            self.zobrist_hash ^= self.zobrist_hasher.hash_white_ks_castling_rights
        }
        if wqs != self.white_qs_castling_rights {
            self.white_qs_castling_rights = wqs;
            self.zobrist_hash ^= self.zobrist_hasher.hash_white_qs_castling_rights
        }
        if bks != self.black_ks_castling_rights {
            self.black_ks_castling_rights = bks;
            self.zobrist_hash ^= self.zobrist_hasher.hash_black_ks_castling_rights
        }
        if bqs != self.black_qs_castling_rights {
            self.black_qs_castling_rights = bqs;
            self.zobrist_hash ^= self.zobrist_hasher.hash_black_qs_castling_rights
        }

        // If this was a castling move, move the rook back.
        // Note that this is the same code block as in make_move (except
        // setting the castling booleans) because of the symmetry in moves.
        if last_move.piece == pieces::KING {
            if last_move.start_square == 4 && last_move.end_square == 6 {
                self.bb_pieces[my_color][pieces::ROOK] ^= bitboard::BB_WKS_CASTLING_ROOKS_FROM_TO;
                self.bb_side[my_color] ^= bitboard::BB_WKS_CASTLING_ROOKS_FROM_TO;
                self.bb_occupied_squares ^= bitboard::BB_WKS_CASTLING_ROOKS_FROM_TO;
                self.bb_empty_squares ^= bitboard::BB_WKS_CASTLING_ROOKS_FROM_TO;
                self.zobrist_hash ^= self.zobrist_hasher.hash_piece[7][my_color][pieces::ROOK];
                self.zobrist_hash ^= self.zobrist_hasher.hash_piece[5][my_color][pieces::ROOK];
            } else if last_move.start_square == 4 && last_move.end_square == 2 {
                self.bb_pieces[my_color][pieces::ROOK] ^= bitboard::BB_WQS_CASTLING_ROOKS_FROM_TO;
                self.bb_side[my_color] ^= bitboard::BB_WQS_CASTLING_ROOKS_FROM_TO;
                self.bb_occupied_squares ^= bitboard::BB_WQS_CASTLING_ROOKS_FROM_TO;
                self.bb_empty_squares ^= bitboard::BB_WQS_CASTLING_ROOKS_FROM_TO;
                self.zobrist_hash ^= self.zobrist_hasher.hash_piece[0][my_color][pieces::ROOK];
                self.zobrist_hash ^= self.zobrist_hasher.hash_piece[3][my_color][pieces::ROOK];
            } else if last_move.start_square == 60 && last_move.end_square == 62 {
                self.bb_pieces[my_color][pieces::ROOK] ^= bitboard::BB_BKS_CASTLING_ROOKS_FROM_TO;
                self.bb_side[my_color] ^= bitboard::BB_BKS_CASTLING_ROOKS_FROM_TO;
                self.bb_occupied_squares ^= bitboard::BB_BKS_CASTLING_ROOKS_FROM_TO;
                self.bb_empty_squares ^= bitboard::BB_BKS_CASTLING_ROOKS_FROM_TO;
                self.zobrist_hash ^= self.zobrist_hasher.hash_piece[63][my_color][pieces::ROOK];
                self.zobrist_hash ^= self.zobrist_hasher.hash_piece[61][my_color][pieces::ROOK];
            } else if last_move.start_square == 60 && last_move.end_square == 58 {
                self.bb_pieces[my_color][pieces::ROOK] ^= bitboard::BB_BQS_CASTLING_ROOKS_FROM_TO;
                self.bb_side[my_color] ^= bitboard::BB_BQS_CASTLING_ROOKS_FROM_TO;
                self.bb_occupied_squares ^= bitboard::BB_BQS_CASTLING_ROOKS_FROM_TO;
                self.bb_empty_squares ^= bitboard::BB_BQS_CASTLING_ROOKS_FROM_TO;
                self.zobrist_hash ^= self.zobrist_hasher.hash_piece[56][my_color][pieces::ROOK];
                self.zobrist_hash ^= self.zobrist_hasher.hash_piece[59][my_color][pieces::ROOK];
            }
        }

        // Bitboards representing to and from squares
        let from_bb = bitboard::to_bb(last_move.start_square);
        let to_bb = bitboard::to_bb(last_move.end_square);
        let from_to_bb = from_bb ^ to_bb;

        // Undo any promotion.  For this step, we just change the queen back
        // to a pawn (we don't change it's board location yet).
        if let Some(p) = last_move.promotion_square {
            // TODO: Be more explicit about queen promotions; this currently
            // will assume the caller wants a queen promotion if not otherwise
            // specified.
            let pp = if let Some(s) = last_move.promotion_piece {s} else {pieces::QUEEN};
            self.bb_pieces[my_color][pieces::PAWN] ^= to_bb;
            self.bb_pieces[my_color][pp] ^= to_bb;
            self.zobrist_hash ^= self.zobrist_hasher.hash_piece[p][my_color][pieces::PAWN];
            self.zobrist_hash ^= self.zobrist_hasher.hash_piece[p][my_color][pp];
        }

        // Handle potential captures
        if let Some(cp) = last_move.captured_piece {
            // A capture occured
            if last_move.is_en_passant {
                // Add the captured pawn back to the board
                let captured_pawn_square: usize = if self.whites_turn {file_rank_to_square(end_file, end_rank-1)} else {file_rank_to_square(end_file, end_rank+1)};
                let captured_pawn_square_bb = bitboard::to_bb(captured_pawn_square);
                self.bb_pieces[opp_color][cp] ^= captured_pawn_square_bb;
                self.bb_side[opp_color] ^= captured_pawn_square_bb;
                self.bb_occupied_squares ^= captured_pawn_square_bb;
                self.bb_occupied_squares ^= from_to_bb;
                self.bb_empty_squares ^= captured_pawn_square_bb;
                self.bb_empty_squares ^= from_to_bb;
                self.zobrist_hash ^= self.zobrist_hasher.hash_piece[captured_pawn_square][opp_color][cp];
            } else {
                // Add the captured piece back to the board
                self.bb_pieces[opp_color][cp] ^= to_bb;
                self.bb_side[opp_color] ^= to_bb;
                self.bb_occupied_squares ^= from_bb;
                self.bb_empty_squares ^= from_bb;
                self.zobrist_hash ^= self.zobrist_hasher.hash_piece[last_move.end_square][opp_color][cp];
            }
        } else {
            // There was no capture; this is a "quiet" move
            self.bb_occupied_squares ^= from_to_bb;
            self.bb_empty_squares ^= from_to_bb;
        }

        // Move the source back
        self.bb_pieces[my_color][last_move.piece] ^= from_to_bb;
        self.bb_side[my_color] ^= from_to_bb;
        self.zobrist_hash ^= self.zobrist_hasher.hash_piece[last_move.end_square][my_color][last_move.piece];
        self.zobrist_hash ^= self.zobrist_hasher.hash_piece[last_move.start_square][my_color][last_move.piece];

    }

    // Return a tuple representing the color and piece on a given square.
    // The will return None if the square is empty.
    pub fn get_color_and_piece_on_square(&self, square: usize) -> Option<(usize, usize)> {

        // Apply bitboards one by one to see if we get a hit
        let square_bb = bitboard::to_bb(square);
        for c in 0..2 {
            for p in 0..6 {
                if bitboard::pop_count(square_bb & self.bb_pieces[c][p]) > 0 {
                    return Some((c, p))
                }
            }
        }
        None

    }

    // Print the board
    #[allow(dead_code)]
    pub fn print(&self, use_unicode: bool) {
        let mut char_board = [['.'; 8]; 8];
        let mut index = 0;
        for (color, _) in self.bb_pieces.iter().enumerate() {
            for (piece, bb) in self.bb_pieces[color].iter().enumerate() {
                for i in (0..64).map (|n| (bb >> n) & 1) {
                    let c = match std::char::from_digit(i as u32, 10) {
                        Some(s) => s,
                        None => panic!("Error in bitboard representation"),
                    };
                    if c == '1' {
                        if use_unicode {
                            char_board[(7 - index / 8) as usize][(index % 8) as usize] = pieces::PIECE_ID_TO_CHAR_UNICODE[color][piece];
                        } else {
                            char_board[(7 - index / 8) as usize][(index % 8) as usize] = pieces::PIECE_ID_TO_CHAR[color][piece];
                        }
                    }
                    index += 1;
                }
                index = 0;
            }
        }
        for (rank, cs) in char_board.iter().enumerate() {
            print!("{}  ", 8 - rank);
            for cell in cs {
                print!("{} ", cell);
            }
            println!();
        }
        println!("   a b c d e f g h");
    }

    // Print the game state, for debugging purposes
    #[allow(dead_code)]
    pub fn print_debug(&self) {
        println!("----------------- DEBUG STATE -----------------");
        println!("BOARD STATE");
        self.print(false);
        println!("OTHER STATE");
        println!("   move_history: {:?}", self.move_history);
        println!("   zobrist_history: {:?}", self.zobrist_history);
        println!("   whites_turn: {}", self.whites_turn);
        println!("   white_ks_castling_rights: {}", self.white_ks_castling_rights);
        println!("   white_qs_castling_rights: {}", self.white_qs_castling_rights);
        println!("   black_ks_castling_rights: {}", self.black_ks_castling_rights);
        println!("   black_qs_castling_rights: {}", self.black_qs_castling_rights);
        println!("   en_passant_rights: {:?}", self.en_passant_rights);
        println!("   zobrist_hash: {}", self.zobrist_hash);
        println!("-------------- END DEBUG STATE ----------------");
    }

}

// =====================================
//             UNIT TESTS
// =====================================

#[cfg(test)]
mod tests {

    use crate::pieces;

    use super::ChessBoard;

    #[test]
    fn test_make_and_unmake_move() {
        // 1. e4 d5 2. exd5 c5 3. dxc6 Nf6 4. c7 e5 5. a4 Ba3 6. Rxa3 O-O 7. cxb8=N Rxb8
        let test_game = [(12, 28), (51, 35), (28, 35), (50, 34), (35, 42), (62, 45), (42, 50), (52, 36), (8, 24), (61, 16), (0, 16), (60, 62), (50, 57), (56, 57)];
        let mut board = ChessBoard::new();
        board.new_game();
        let initial_hash = board.zobrist_hash;
        // Make moves, checking hashes
        for (start_square, end_square) in test_game {
            if (start_square, end_square) == (50, 57) {
                board.make_move(start_square, end_square, Some(pieces::KNIGHT));
            } else {
                board.make_move(start_square, end_square, None);
            }
            assert_eq!(board.zobrist_hash, board.zobrist_hasher.full_hash(&board));
        }
        // Unmake moves, checking hashes
        while board.move_history.len() > 0 {
            board.unmake_move();
            assert_eq!(board.zobrist_hash, board.zobrist_hasher.full_hash(&board));
        }
        // Ensure initial hash matches
        assert_eq!(initial_hash, board.zobrist_hash);
    }
    
}