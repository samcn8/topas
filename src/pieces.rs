//! This module contains information and helpful constants related
//! to game pieces.

pub const PAWN: usize = 0;
pub const KNIGHT: usize = 1;
pub const BISHOP: usize = 2;
pub const ROOK: usize = 3;
pub const QUEEN: usize = 4;
pub const KING: usize = 5;

pub const COLOR_WHITE: usize = 0;
pub const COLOR_BLACK: usize = 1;

pub const PIECE_ID_TO_CHAR: [[char; 6]; 2] = [['P', 'N', 'B', 'R', 'Q', 'K'],
                                              ['p', 'n', 'b', 'r', 'q', 'k']];
