//! This module implements Zobrist hashing for use in transposition tables.
//! See https://en.wikipedia.org/wiki/Zobrist_hashing for more information.

use rand::Rng;
use crate::chess_board;
use crate::bitboard;

pub struct ZobristHasher {

    // 3D array containing a random 64-bit value for [square][color][piece].
    // This is stack-allocated and takes 8B * 64*6*2 = ~6KB of memory.
    pub hash_piece: [[[u64; 6]; 2]; 64],

    // Hash applied when it's black's turn
    pub hash_blacks_turn: u64,

    // Hash applied for various castling rights
    pub hash_white_ks_castling_rights: u64,
    pub hash_white_qs_castling_rights: u64,
    pub hash_black_ks_castling_rights: u64,
    pub hash_black_qs_castling_rights: u64,

    // Has applied for the en passant square.  Note that we only have
    // to apply the file to make this disambiguous (so, 8 total values).
    pub hash_en_passant: [u64; 8],
}

impl ZobristHasher {

    // Construct a stack-allocated ZobristHasher
    pub fn new() -> ZobristHasher {

        // Initialize everything with random values
        let mut rng = rand::thread_rng();
        let mut hash_piece = [[[0; 6]; 2]; 64];
        for square in 0..64 {
            for color in 0..2 {
                for piece in 0..6 {
                    hash_piece[square][color][piece] = rng.gen::<u64>();
                }
            }
        }
        let mut hash_en_passant: [u64; 8] = [0; 8];
        for e in 0..8 {
            hash_en_passant[e] = rng.gen::<u64>();
        }
        ZobristHasher {
            hash_piece,
            hash_blacks_turn: rng.gen::<u64>(),
            hash_white_ks_castling_rights: rng.gen::<u64>(),
            hash_white_qs_castling_rights: rng.gen::<u64>(),
            hash_black_ks_castling_rights: rng.gen::<u64>(),
            hash_black_qs_castling_rights: rng.gen::<u64>(),
            hash_en_passant,
        }

    }

    // This is typically only called at the beginning of the game.
    // Normally, the hash will be incrementally updated directly on
    // the fields which is much faster.
    pub fn full_hash(&self, board: &chess_board::ChessBoard) -> u64 {

        // Hash the state of the board with our saved random values
        let mut h: u64 = 0;
        if !board.whites_turn {
            h ^= self.hash_blacks_turn;
        }
        for (color, _) in board.bb_pieces.iter().enumerate() {
            for (piece, bb) in board.bb_pieces[color].iter().enumerate() {
                for square in bitboard::occupied_squares(*bb) {
                    h ^= self.hash_piece[square][color][piece];
                }
            }
        }
        if board.white_ks_castling_rights {
            h ^= self.hash_white_ks_castling_rights;
        }
        if board.white_qs_castling_rights {
            h ^= self.hash_white_qs_castling_rights;
        }
        if board.black_ks_castling_rights {
            h ^= self.hash_black_ks_castling_rights;
        }
        if board.black_qs_castling_rights {
            h ^= self.hash_black_qs_castling_rights;
        }
        if let Some(s) = board.en_passant_rights {
            h ^= self.hash_en_passant[s % 8];
        }
        h
    }

}