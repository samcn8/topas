//! This module contains functions related to bitboard operations and
//! lookup tables.
//! The vast majority of the algorithms in this file are from descriptions
//! and psuedocode from https://www.chessprogramming.org.
//! Note that 'for' loops are not allow in Rust const fn (yet).  Hence,
//! these functions will often use a simple 'loop' with breaks.
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

use crate::pieces;

// These values are used in the De Bruijn Multiplication algorithm to find the
// least signficiant 1 bit (LS1B) and most significant 1 bit (MS1B) of an integer.
const LSB_INDEX64_LOOKUP: [usize; 64] = [
    0,  1, 48,  2, 57, 49, 28,  3,
   61, 58, 50, 42, 38, 29, 17,  4,
   62, 55, 59, 36, 53, 51, 43, 22,
   45, 39, 33, 30, 24, 18, 12,  5,
   63, 47, 56, 27, 60, 41, 37, 16,
   54, 35, 52, 21, 44, 32, 23, 11,
   46, 26, 40, 15, 34, 20, 31, 10,
   25, 14, 19,  9, 13,  8,  7,  6
];
const MSB_INDEX64_LOOKUP: [usize; 64] = [
    0, 47,  1, 56, 48, 27,  2, 60,
   57, 49, 41, 37, 28, 16,  3, 61,
   54, 58, 35, 52, 50, 42, 21, 44,
   38, 32, 29, 23, 17, 11,  4, 62,
   46, 55, 26, 59, 40, 36, 15, 53,
   34, 51, 20, 43, 31, 22, 10, 45,
   25, 39, 14, 33, 19, 30,  9, 24,
   13, 18,  8, 12,  7,  6,  5, 63
];
const DEBRUIJN64: u64 = 0x03f79d71b4cb0a89;

// Bitboards for rook movement during castling.  These set 1 bits for the
// from and to position of the rooks before and after a castle.  So,
// each bitboard here has two "1 bits" set.
pub const BB_WKS_CASTLING_ROOKS_FROM_TO: u64 = 0x00000000000000a0;
pub const BB_WQS_CASTLING_ROOKS_FROM_TO: u64 = 0x0000000000000009;
pub const BB_BKS_CASTLING_ROOKS_FROM_TO: u64 = 0xa000000000000000;
pub const BB_BQS_CASTLING_ROOKS_FROM_TO: u64 = 0x0900000000000000;

// Bitboards related to lines, used for move computation
pub const BB_FILES: [u64; 8] = [
    0x0101010101010101, // A file
    0x0202020202020202, // B file
    0x0404040404040404, // C file
    0x0808080808080808, // D file
    0x1010101010101010, // E file
    0x2020202020202020, // F file
    0x4040404040404040, // G file
    0x8080808080808080  // H file
];
pub const BB_4RANK: u64 = 0x00000000FF000000;
pub const BB_5RANK: u64 = 0x000000FF00000000;
pub const BB_NOT_AFILE: u64 = 0xfefefefefefefefe;
pub const BB_NOT_HFILE: u64 = 0x7f7f7f7f7f7f7f7f;
pub const BB_MAIN_DIAGONAL: u64 = 0x8040201008040201;
const BB_MAIN_ANTIDIAGONAL: u64 = 0x0102040810204080;

// Bitboards related to castling.  These set the squares between
// the king and appropriate rook to 1.  We'll AND these squares
// with occupancy and then check for a 0 bitboard to determine
// if the line between the king and rook is clear
pub const BB_WKS_BETWEEN: u64 = 0x0000000000000060;
pub const BB_WQS_BETWEEN: u64 = 0x000000000000000e;
pub const BB_BKS_BETWEEN: u64 = 0x6000000000000000;
pub const BB_BQS_BETWEEN: u64 = 0x0e00000000000000;

// Bitboards related to castling, setting the ending positions
// of the king for various castling moves.
pub const BB_WKS_KING_END: u64 = to_bb(6);
pub const BB_WQS_KING_END: u64 = to_bb(2);
pub const BB_BKS_KING_END: u64 = to_bb(62);
pub const BB_BQS_KING_END: u64 = to_bb(58);

// Line masks for sliding piece computation.
// See https://www.chessprogramming.org/On_an_empty_Board
const fn compute_rank_masks() -> [u64; 64] {
    let mut masks = [0; 64];
    let mut square = 0;
    loop {
        masks[square] = 0xff_u64.wrapping_shl(square as u32 & 56);
        square += 1;
        if square >= 64 {
            break;
        }
    }
    masks
}
pub const BB_RANK_MASK: [u64; 64] = compute_rank_masks();
const fn compute_diagonal_masks() -> [u64; 64] {
    let mut masks = [0; 64];
    let mut square = 0;
    loop {
        let diag: i32 = 8 * (square & 7) - (square & 56);
        let nort = diag.wrapping_neg() & diag.wrapping_shr(31);
        let sout = diag & diag.wrapping_neg().wrapping_shr(31);
        masks[square as usize] = BB_MAIN_DIAGONAL.wrapping_shr(sout as u32).wrapping_shl(nort as u32);
        square += 1;
        if square >= 64 {
            break;
        }
    }
    masks
}
pub const BB_DIAGONAL_MASK: [u64; 64] = compute_diagonal_masks();
const fn compute_antidiagonal_masks() -> [u64; 64] {
    let mut masks = [0; 64];
    let mut square = 0;
    loop {
        let diag: i32 = 56 - 8 * (square & 7) - (square & 56);
        let nort = diag.wrapping_neg() & diag.wrapping_shr(31);
        let sout = diag & diag.wrapping_neg().wrapping_shr(31);
        masks[square as usize] = BB_MAIN_ANTIDIAGONAL.wrapping_shr(sout as u32).wrapping_shl(nort as u32);
        square += 1;
        if square >= 64 {
            break;
        }
    }
    masks
}
pub const BB_ANTIDIAGONAL_MASK: [u64; 64] = compute_antidiagonal_masks();

// First rank attack lookup table.  This is used by Kindergarten bitboards
// to compute sliding piece attacks.
const FULL_RAY: i32 = 0;
const WEST_RAY: i32 = 1;
const EAST_RAY: i32 = 2;
const fn compute_first_rank_west_attacks_for_square(square: u8, occ: u8) -> u8 {
    let square_bb = 1u8.wrapping_shl(square as u32);
    let mut west_attacks = square_bb - 1;
    let west_blockers = west_attacks & occ;
    if let Some(b) = bit_scan_reverse(west_blockers as u64) {
        let west_main_blocker = 1u8.wrapping_shl(b as u32);
        let west_passed_blocker = west_main_blocker - 1;
        west_attacks ^= west_passed_blocker;
    }
    west_attacks
}
const fn compute_first_rank_east_attacks_for_square(square: u8, occ: u8) -> u8 {
    let square_bb = 1u8.wrapping_shl(square as u32);
    let mut east_attacks = !square_bb & !(square_bb - 1);
    let east_blockers = east_attacks & occ;
    if let Some(b) = bit_scan_forward(east_blockers as u64) {
        let east_main_blocker = 1u8.wrapping_shl(b as u32);
        let east_passed_blocker = !east_main_blocker & !(east_main_blocker - 1);
        east_attacks ^= east_passed_blocker;
    }
    east_attacks
}
const fn compute_first_rank_attacks(ray: i32) -> [[u8; 256]; 8] {
    let mut attacks: [[u8; 256]; 8] = [[0; 256]; 8];
    let mut square: u8 = 0;
    let mut occ: u8 = 0;
    loop {
        loop {
            if ray == FULL_RAY {
                attacks[square as usize][occ as usize] = compute_first_rank_west_attacks_for_square(square, occ) ^ compute_first_rank_east_attacks_for_square(square, occ);
            } else if ray == WEST_RAY {
                attacks[square as usize][occ as usize] = compute_first_rank_west_attacks_for_square(square, occ);
            } else if ray == EAST_RAY {
                attacks[square as usize][occ as usize] = compute_first_rank_east_attacks_for_square(square, occ);
            }
            if occ >= 255 {
                break;
            }
            occ += 1;
        }
        occ = 0;
        square += 1;
        if square >= 8 {
            break;
        }
    }
    attacks
}
pub const BB_FIRST_RANK_ATTACKS: [[u8; 256]; 8] = compute_first_rank_attacks(FULL_RAY);

// For SEE capture scoring, it's easier for us to have "east" and "west"
// first rank attack rays.  So, store these seperately.
pub const BB_FIRST_RANK_WEST_ATTACKS: [[u8; 256]; 8] = compute_first_rank_attacks(WEST_RAY);
pub const BB_FIRST_RANK_EAST_ATTACKS: [[u8; 256]; 8] = compute_first_rank_attacks(EAST_RAY);

// Compute single step bitboard functions
pub const fn south_one(bb: u64) -> u64 {bb.wrapping_shr(8)}
pub const fn north_one(bb: u64) -> u64 {bb.wrapping_shl(8)}
const fn east_one(bb: u64) -> u64 {bb.wrapping_shl(1) & BB_NOT_AFILE}
const fn north_east_one(bb: u64) -> u64 {bb.wrapping_shl(9) & BB_NOT_AFILE}
const fn south_east_one(bb: u64) -> u64 {bb.wrapping_shr(7) & BB_NOT_AFILE}
const fn west_one(bb: u64) -> u64 {bb.wrapping_shr(1) & BB_NOT_HFILE}
const fn south_west_one(bb: u64) -> u64 {bb.wrapping_shr(9) & BB_NOT_HFILE}
const fn north_west_one(bb: u64) -> u64 {bb.wrapping_shl(7) & BB_NOT_HFILE}

// Compute pawn fill bitboards
const fn north_fill(mut bb: u64) -> u64 {
    bb |= bb.wrapping_shl(8);
    bb |= bb.wrapping_shl(16);
    bb |= bb.wrapping_shl(32);
    bb
}
const fn south_fill(mut bb: u64) -> u64 {
    bb |= bb.wrapping_shr(8);
    bb |= bb.wrapping_shr(16);
    bb |= bb.wrapping_shr(32);
    bb
}

// Bitboards representing pawn front spans, used for passed pawn detection
const fn compute_pawn_front_spans_color(color: usize) -> [u64; 64] {
    let mut pawn_front_spans: [u64; 64] = [0; 64];
    let mut square = 0;
    let mut bb_pawn = 1;
    loop {
        if color == pieces::COLOR_WHITE {
            pawn_front_spans[square as usize] = north_one(north_fill(bb_pawn)) | north_east_one(north_fill(bb_pawn)) | north_west_one(north_fill(bb_pawn));
        } else {
            pawn_front_spans[square as usize] = south_one(south_fill(bb_pawn)) | south_east_one(south_fill(bb_pawn)) | south_west_one(south_fill(bb_pawn));
        }
        bb_pawn = bb_pawn.wrapping_shl(1);
        square += 1;
        if square >= 64 {
            break;
        }
    }
    pawn_front_spans
}
const fn compute_pawn_front_spans() -> [[u64; 64]; 2] {
    let mut pawn_front_spans: [[u64; 64]; 2] = [[0; 64]; 2];
    pawn_front_spans[pieces::COLOR_WHITE] = compute_pawn_front_spans_color(pieces::COLOR_WHITE);
    pawn_front_spans[pieces::COLOR_BLACK] = compute_pawn_front_spans_color(pieces::COLOR_BLACK);
    pawn_front_spans
}
pub const BB_PAWN_FRONT_SPAN: [[u64; 64]; 2] = compute_pawn_front_spans();

// Bitboards representing attacks of pawns (per color), for fast lookup
const fn compute_pawn_attacks_color(color: usize) -> [u64; 64] {
    let mut pawn_attacks: [u64; 64] = [0; 64];
    let mut square = 0;
    let mut bb_pawn = 1;
    loop {
        if color == pieces::COLOR_WHITE {
            pawn_attacks[square as usize] = north_east_one(bb_pawn) | north_west_one(bb_pawn);
        } else {
            pawn_attacks[square as usize] = south_east_one(bb_pawn) | south_west_one(bb_pawn);
        }
        bb_pawn = bb_pawn.wrapping_shl(1);
        square += 1;
        if square >= 64 {
            break;
        }
    }
    pawn_attacks
}
const fn compute_pawn_attacks() -> [[u64; 64]; 2] {
    let mut pawn_attacks: [[u64; 64]; 2] = [[0; 64]; 2];
    pawn_attacks[pieces::COLOR_WHITE] = compute_pawn_attacks_color(pieces::COLOR_WHITE);
    pawn_attacks[pieces::COLOR_BLACK] = compute_pawn_attacks_color(pieces::COLOR_BLACK);
    pawn_attacks
}
pub const BB_PAWN_ATTACKS: [[u64; 64]; 2] = compute_pawn_attacks();

// Bitboards representing attacks of knights, for fast lookup
const fn compute_knight_attacks() -> [u64; 64] {
    let mut knight_attacks: [u64; 64] = [0; 64];
    let mut square = 0;
    let mut bb_knight = 1;
    loop {
        knight_attacks[square as usize] = {
            let mut east = east_one(bb_knight);
            let mut west = west_one(bb_knight);
            let mut attacks = (east | west).wrapping_shl(16);
            attacks |= (east | west).wrapping_shr(16);
            east = east_one(east);
            west = west_one(west);
            attacks |= (east | west).wrapping_shl(8);
            attacks |= (east | west).wrapping_shr(8);
            attacks
        };
        bb_knight = bb_knight.wrapping_shl(1);
        square += 1;
        if square >= 64 {
            break;
        }
    }
    knight_attacks
}
pub const BB_KNIGHT_ATTACKS: [u64; 64] = compute_knight_attacks();

// Bitboards representing attacks of kings, for fast lookup
const fn compute_king_attacks() -> [u64; 64] {
    let mut king_attacks: [u64; 64] = [0; 64];
    let mut square = 0;
    let mut bb_king = 1;
    loop {
        king_attacks[square as usize] = {
            let mut bb_king_temp = bb_king;
            let mut attacks = east_one(bb_king_temp) | west_one(bb_king_temp);
            bb_king_temp |= attacks;
            attacks |= north_one(bb_king_temp) | south_one(bb_king_temp);
            attacks
        };
        bb_king = bb_king.wrapping_shl(1);
        square += 1;
        if square >= 64 {
            break;
        }
    }
    king_attacks
}
pub const BB_KING_ATTACKS: [u64; 64] = compute_king_attacks();

// Create a bitboard with a single 1 in it, at the location of "square".
pub const fn to_bb(square: usize) -> u64 {
    1u64.wrapping_shl(square as u32)
}

// Return the number of 1's set in a bitboard.
pub fn pop_count(bb: u64) -> u8 {
    bb.count_ones() as u8
}

// Finds the least signficiant 1 bit (LS1B) of a bitboard
// using De Bruijn Multiplication.
pub const fn bit_scan_forward(bb: u64) -> Option<usize> {
    if bb != 0 {
        Some(LSB_INDEX64_LOOKUP[(bb & bb.wrapping_neg()).wrapping_mul(DEBRUIJN64).wrapping_shr(58) as usize])
    } else {
        None
    }
}

// Finds the most signficiant 1 bit (MS1B) of a bitboard
// using De Bruijn Multiplication.
const fn bit_scan_reverse(mut bb: u64) -> Option<usize> {
    if bb != 0 {
        bb |= bb.wrapping_shr(1);
        bb |= bb.wrapping_shr(2);
        bb |= bb.wrapping_shr(4);
        bb |= bb.wrapping_shr(8);
        bb |= bb.wrapping_shr(16);
        bb |= bb.wrapping_shr(32);
        Some(MSB_INDEX64_LOOKUP[bb.wrapping_mul(DEBRUIJN64).wrapping_shr(58) as usize])
    } else {
        None
    }
 }


// Given a bitboard, return a list of the locations of all 1's set.
// These typically represent "occupied" squares in a bitboard.
// Note the bitboard is copied here so we're not modifying the
// caller's passed-in bitboard.
pub fn occupied_squares(mut bb: u64) -> Vec<usize> {
    let mut occupied = Vec::new();
    while bb != 0 {
        // Find least signficiant 1 bit
        let lsb = match bit_scan_forward(bb) {
            Some(s) => s,
            None => panic!("Illegal bitboard"),
        };
        // Clear that bit
        bb &= bb-1; 
        occupied.push(lsb);
    }
    occupied
}

// For debugging
#[allow(dead_code)]
pub fn print(bb: u64) {
    let mut char_board = [['.'; 8]; 8];
    let mut index = 0;
    for i in (0..64).map (|n| (bb >> n) & 1) {
        let c = match std::char::from_digit(i as u32, 10) {
            Some(s) => s,
            None => panic!("Error in bitboard representation"),
        };
        if c == '1' {
            char_board[(7 - index / 8) as usize][(index % 8) as usize] = '1';
        }
        index += 1;
    }
    for cs in char_board {
        let str: String = cs.iter().collect();
        println!("   {}", str);
    }
}