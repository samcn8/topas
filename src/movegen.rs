//! This module contains functions related to piece movement and
//! move legality checking.

use crate::bitboard;
use crate::chess_board;
use crate::pieces;

#[derive(Debug)]
pub struct ChessMove {
    
    // Starting square of the piece being moved
    pub start_square: usize,

    // Ending square of the piece being moved
    pub end_square: usize,

    // Piece type of the piece being moved
    pub piece: usize,

    // Captured piece, or None if no capture made
    pub captured_piece: Option<usize>,

    // Priority of the move, only relavant for search
    pub priority: i32,

    // Whether or not this is an en passant capture
    pub is_en_passant: bool,

}

// Converts a standard square position string into a square ID.
// For instance, "a3" -> 3
fn convert_square_str_into_id(move_str: &str) -> usize {
    let file = if let Some(e) = move_str.chars().nth(0) {e} else {panic!("Invalid move string - file")};
    let file = if let Some(e) = "abcdebgh".find(file) {e as usize} else {panic!("Invalid move string - file")};
    let rank = if let Some(e) = move_str.chars().nth(1) {e} else {panic!("Invalid move string - rank")};
    let rank = if let Some(e) = rank.to_digit(10) {(e-1) as usize} else {panic!("Invalid move string - rank")};
    rank * 8 + file
}

// Converts a UCI-style move list (long algebraic notation without
// piece names) into a vector of (start square, end square) tuples.
pub fn convert_moves_str_into_list(move_str: &str) -> Vec<(usize, usize)> {
    let mut moves = Vec::new();
    for m in move_str.split_whitespace() {
        let start_square = convert_square_str_into_id(&m[0..2]);
        let end_square = convert_square_str_into_id(&m[2..4]);
        // TODO - handle a potential 5th character indicating a promoted
        //        piece type; right now, this assumes a queen
        moves.push((start_square, end_square));
    }
    moves
}

// Get any pawn push moves for a color from a starting location.
fn get_pawn_push_targets_bb(color: usize, empty: u64, square: usize) -> u64 {
    let pawn_bb = bitboard::to_bb(square);
    let single_push_bb = if color == pieces::COLOR_WHITE {bitboard::north_one(pawn_bb) & empty} else {bitboard::south_one(pawn_bb) & empty};
    let double_push_bb = if color == pieces::COLOR_WHITE {bitboard::north_one(single_push_bb) & empty & bitboard::BB_4RANK} else {bitboard::south_one(single_push_bb) & empty & bitboard::BB_5RANK};
    single_push_bb | double_push_bb
}

// Get any king target square related to castling
fn get_castling_king_targets_bb(board: &chess_board::ChessBoard, color: usize, occ: u64) -> u64 {
    let mut king_castling_bb: u64 = 0;
    if color == pieces::COLOR_WHITE {
        // Ensure we have appropriate castling rights and there are
        // no pieces between the king and rook
        if board.white_ks_castling_rights {
            if bitboard::BB_WKS_BETWEEN & occ == 0 {
                king_castling_bb |= bitboard::BB_WKS_KING_END;
            }
        }
        if board.white_qs_castling_rights {
            if bitboard::BB_WQS_BETWEEN & occ == 0 {
                king_castling_bb |= bitboard::BB_WQS_KING_END;
            }
        }
    } else {
        // Ensure we have appropriate castling rights and there are
        // no pieces between the king and rook
        if board.black_ks_castling_rights {
            if bitboard::BB_BKS_BETWEEN & occ == 0 {
                king_castling_bb |= bitboard::BB_BKS_KING_END;
            }
        }
        if board.black_qs_castling_rights {
            if bitboard::BB_BQS_BETWEEN & occ == 0 {
                king_castling_bb |= bitboard::BB_BQS_KING_END;
            }
        }
    }
    king_castling_bb
}

// Get all diagonal attacks (bottom left to top right) from a starting
// location.
// Portion 0 is the entire ray
// Portion 1 is the southern part of the ray (mapping west in first rank)
// Portion 2 is the northern part of the ray (mapping east in first rank)
pub fn get_diagonal_attacks_bb(occ: u64, square: usize, portion: i32) -> u64 {
    let tmp_occ = (bitboard::BB_DIAGONAL_MASK[square] & occ).wrapping_mul(bitboard::BB_FILES[0]).wrapping_shr(56);
    let first_rank_bb;
    if portion == 1 {
        first_rank_bb = bitboard::BB_FIRST_RANK_WEST_ATTACKS[square & 7][tmp_occ as usize] as u64
    } else if portion == 2 {
        first_rank_bb = bitboard::BB_FIRST_RANK_EAST_ATTACKS[square & 7][tmp_occ as usize] as u64
    } else {
        first_rank_bb = bitboard::BB_FIRST_RANK_ATTACKS[square & 7][tmp_occ as usize] as u64
    }
    bitboard::BB_DIAGONAL_MASK[square] & bitboard::BB_FILES[0].wrapping_mul(first_rank_bb)
}

// Get all anti-diagonal attacks (top left to bottom right) from a starting
// location.
// Portion 0 is the entire ray
// Portion 1 is the northern part of the ray (mapping west in first rank)
// Portion 2 is the southern part of the ray (mapping east in first rank)
pub fn get_antidiagonal_attacks_bb(occ: u64, square: usize, portion: i32) -> u64 {
    let tmp_occ = (bitboard::BB_ANTIDIAGONAL_MASK[square] & occ).wrapping_mul(bitboard::BB_FILES[0]).wrapping_shr(56);
    let first_rank_bb;
    if portion == 1 {
        first_rank_bb = bitboard::BB_FIRST_RANK_WEST_ATTACKS[square & 7][tmp_occ as usize] as u64
    } else if portion == 2 {
        first_rank_bb = bitboard::BB_FIRST_RANK_EAST_ATTACKS[square & 7][tmp_occ as usize] as u64
    } else {
        first_rank_bb = bitboard::BB_FIRST_RANK_ATTACKS[square & 7][tmp_occ as usize] as u64
    }
    bitboard::BB_ANTIDIAGONAL_MASK[square] & bitboard::BB_FILES[0].wrapping_mul(first_rank_bb)
}

// Get all rank attacks from a starting location
// Portion 0 is the entire ray
// Portion 1 is the western part of the ray (mapping west in first rank)
// Portion 2 is the eastern part of the ray (mapping east in first rank)
pub fn get_rank_attacks_bb(occ: u64, square: usize, portion: i32) -> u64 {
    let tmp_occ = (bitboard::BB_RANK_MASK[square] & occ).wrapping_mul(bitboard::BB_FILES[0]).wrapping_shr(56);
    let first_rank_bb;
    if portion == 1 {
        first_rank_bb = bitboard::BB_FIRST_RANK_WEST_ATTACKS[square & 7][tmp_occ as usize] as u64
    } else if portion == 2 {
        first_rank_bb = bitboard::BB_FIRST_RANK_EAST_ATTACKS[square & 7][tmp_occ as usize] as u64
    } else {
        first_rank_bb = bitboard::BB_FIRST_RANK_ATTACKS[square & 7][tmp_occ as usize] as u64
    }
    bitboard::BB_RANK_MASK[square] & bitboard::BB_FILES[0].wrapping_mul(first_rank_bb)
}

// Get all file attacks from a starting location
// Portion 0 is the entire ray
// Portion 1 is the northern part of the ray (mapping west in first rank)
// Portion 2 is the southern part of the ray (mapping east in first rank)
pub fn get_file_attacks_bb(occ: u64, square: usize, portion: i32) -> u64 {
    let tmp_square = square & 7;
    let mut tmp_occ = bitboard::BB_FILES[0] & occ.wrapping_shr(tmp_square as u32);
    tmp_occ = bitboard::BB_MAIN_DIAGONAL.wrapping_mul(tmp_occ).wrapping_shr(56);
    let index = (square ^ 56).wrapping_shr(3);
    let first_rank_bb;
    if portion == 1 {
        first_rank_bb = bitboard::BB_FIRST_RANK_WEST_ATTACKS[index][tmp_occ as usize] as u64
    } else if portion == 2 {
        first_rank_bb = bitboard::BB_FIRST_RANK_EAST_ATTACKS[index][tmp_occ as usize] as u64
    } else {
        first_rank_bb = bitboard::BB_FIRST_RANK_ATTACKS[index][tmp_occ as usize] as u64
    }
    tmp_occ = bitboard::BB_MAIN_DIAGONAL.wrapping_mul(first_rank_bb);
    (bitboard::BB_FILES[7] & tmp_occ).wrapping_shr((tmp_square ^ 7) as u32)
}

// Determine the opponent's piece that is being captured
fn get_opponents_captured_piece(opp_bbs: &Vec<u64>, capture_square: usize, is_en_passant: bool) -> usize {
    if is_en_passant {
        return pieces::PAWN;
    }
    for (opp_piece, opp_bb) in opp_bbs.iter().enumerate() {
        if bitboard::occupied_squares(*opp_bb).contains(&capture_square) {
            return opp_piece;
        }
    }
    panic!("Invalid bitboard; cannot find opponents captured piece");
}

// Generate all psuedo-legal moves for a given color.
// A psuedo-legal move is an otherwise legal move that has not yet been
// checked to determine if it leaves the player's king in check.
pub fn generate_all_psuedo_legal_moves(board: &chess_board::ChessBoard, my_color: usize) -> Vec<ChessMove> {
    
    let mut capture_moves = Vec::new();
    let mut quiet_moves = Vec::new();

    // Get colors
    let opp_color = 1 - my_color;

    // Create the en passant bitboard, which will be 0 if no en passant
    // rights exist
    let mut en_passant_bb = 0;
    if let Some(e) = board.en_passant_rights {
        en_passant_bb = bitboard::to_bb(e);
    }

    // Loop through each of our bitboards to generate a set of pseudo-legal moves
    for (piece, bb) in board.bb_pieces[my_color].iter().enumerate() {
        for square in bitboard::occupied_squares(*bb) {
            
            // Store state regarding an en passant capture
            let mut is_en_passant = false;

            // Get quite (i.e., non-capture) and capture move bitboards for the piece
            let quite_move_bb;
            let capture_move_bb;
            if piece == pieces::PAWN {
                quite_move_bb = get_pawn_push_targets_bb(my_color, board.bb_empty_squares, square);
                if bitboard::BB_PAWN_ATTACKS[my_color][square] & en_passant_bb != 0 {
                    is_en_passant = true;
                }
                capture_move_bb = bitboard::BB_PAWN_ATTACKS[my_color][square] & (board.bb_side[opp_color] | en_passant_bb);
            } else if piece == pieces::KNIGHT {
                quite_move_bb = bitboard::BB_KNIGHT_ATTACKS[square] & board.bb_empty_squares;
                capture_move_bb = bitboard::BB_KNIGHT_ATTACKS[square] & board.bb_side[opp_color];
            } else if piece == pieces::BISHOP {
                let bishop_attacks = get_diagonal_attacks_bb(board.bb_occupied_squares, square, 0) | get_antidiagonal_attacks_bb(board.bb_occupied_squares, square, 0);
                quite_move_bb = bishop_attacks & board.bb_empty_squares;
                capture_move_bb = bishop_attacks & board.bb_side[opp_color];
            } else if piece == pieces::ROOK {
                let rook_attacks = get_rank_attacks_bb(board.bb_occupied_squares, square, 0) | get_file_attacks_bb(board.bb_occupied_squares, square, 0);
                quite_move_bb = rook_attacks & board.bb_empty_squares;
                capture_move_bb = rook_attacks & board.bb_side[opp_color];
            } else if piece == pieces::QUEEN {
                let bishop_attacks = get_diagonal_attacks_bb(board.bb_occupied_squares, square, 0) | get_antidiagonal_attacks_bb(board.bb_occupied_squares, square, 0);
                let rook_attacks = get_rank_attacks_bb(board.bb_occupied_squares, square, 0) | get_file_attacks_bb(board.bb_occupied_squares, square, 0);
                let queen_attacks = bishop_attacks | rook_attacks;
                quite_move_bb = queen_attacks & board.bb_empty_squares;
                capture_move_bb = queen_attacks & board.bb_side[opp_color];
            } else if piece == pieces::KING {
                quite_move_bb = (bitboard::BB_KING_ATTACKS[square] & board.bb_empty_squares) | get_castling_king_targets_bb(board, my_color, board.bb_occupied_squares);
                capture_move_bb = bitboard::BB_KING_ATTACKS[square] & board.bb_side[opp_color];
            } else {
                println!("Invalid piece selection in generate_all_psuedo_legal_moves");
                continue;
            }

            // First get non-capture moves
            for m in bitboard::occupied_squares(quite_move_bb) {
                let cmove = ChessMove {
                    start_square: square,
                    end_square: m,
                    piece,
                    captured_piece: None,
                    priority: 0,
                    is_en_passant: false,
                };
                quiet_moves.push(cmove);
            }

            // Next get capture moves
            for m in bitboard::occupied_squares(capture_move_bb) {
                // Figure out the piece that is being captured
                let cap = get_opponents_captured_piece(&board.bb_pieces[opp_color], m, is_en_passant);
                let cmove = ChessMove {
                    start_square: square,
                    end_square: m,
                    piece,
                    captured_piece: Some(cap),
                    priority: 0,
                    is_en_passant,
                };
                capture_moves.push(cmove);
            }

        }
    }

    // Order capture moves first (by appending quiet moves to the end)
    // This will get re-sorted anyway, but may make the re-sort faster.
    capture_moves.append(&mut quiet_moves);
    capture_moves
}

// Determines whether the king of a given color is in check
fn is_square_attacked_by_side(board: &chess_board::ChessBoard, square: usize, by_side_color: usize) -> bool {
    let pawns = board.bb_pieces[by_side_color][pieces::PAWN];
    if bitboard::BB_PAWN_ATTACKS[1 - by_side_color][square] & pawns != 0 {
        return true;
    }
    let knights = board.bb_pieces[by_side_color][pieces::KNIGHT];
    if bitboard::BB_KNIGHT_ATTACKS[square] & knights != 0 {
        return true;
    }
    let king = board.bb_pieces[by_side_color][pieces::KING];
    if bitboard::BB_KING_ATTACKS[square] & king != 0 {
        return true;
    }
    let bishops_queens = board.bb_pieces[by_side_color][pieces::BISHOP] | board.bb_pieces[by_side_color][pieces::QUEEN];
    if (get_diagonal_attacks_bb(board.bb_occupied_squares, square, 0) | get_antidiagonal_attacks_bb(board.bb_occupied_squares, square, 0)) & bishops_queens != 0 {
        return true;
    }
    let rooks_queens = board.bb_pieces[by_side_color][pieces::ROOK] | board.bb_pieces[by_side_color][pieces::QUEEN];
    if (get_rank_attacks_bb(board.bb_occupied_squares, square, 0) | get_file_attacks_bb(board.bb_occupied_squares, square, 0)) & rooks_queens != 0 {
        return true;
    }
    false
}

// Check whether or not the king of the passed in color is in check
pub fn is_king_in_check(board: &chess_board::ChessBoard, king_color: usize) -> bool {
    let king_square = match bitboard::bit_scan_forward(board.bb_pieces[king_color][pieces::KING]) {
        Some(e) => e,
        None => panic!("Cannot find king on bitboard"),
    };
    is_square_attacked_by_side(&board, king_square, 1 - king_color)
}

// Modify the passed in moves vector to keep only moves that don't leave
// player's king in check.
pub fn retain_only_legal_moves(board: &mut chess_board::ChessBoard, moves: &mut Vec<ChessMove>) {
    let my_color = if board.whites_turn {pieces::COLOR_WHITE} else {pieces::COLOR_BLACK};
    moves.retain(|i| {
        board.make_move(i.start_square, i.end_square);
        let keepit = !is_king_in_check(board, my_color);
        board.unmake_move();
        keepit
    });

}

// =====================================
//             UNIT TESTS
// =====================================

#[cfg(test)]
mod tests {
    
    use crate::chess_board::ChessBoard;
    use super::*;

    fn get_number_of_valid_moves(board: &mut chess_board::ChessBoard, depth: usize) -> u64 {
        if depth == 0 {
            return 1;
        }
        let mut move_count = 0;
        let my_color = if board.whites_turn {pieces::COLOR_WHITE} else {pieces::COLOR_BLACK};
        let mut moves = generate_all_psuedo_legal_moves(&board, my_color);
        retain_only_legal_moves(board, &mut moves);
        for m in moves.iter() {
            board.make_move(m.start_square, m.end_square);
            move_count += get_number_of_valid_moves(board, depth - 1);
            board.unmake_move();
        }
        move_count
    }

    // Test the number of valid moves
    #[test]
    fn test_perft() {
        let results = vec![1, 20, 400, 8902, 197281, 4865609];
        let mut board = ChessBoard::new();
        board.new_game();
        for i in 0..results.len() {
            let moves = get_number_of_valid_moves(&mut board, i);
            assert_eq!(moves, results[i]);
            println!("{} moves at depth {}", moves, i);
        }
    }

    // Test a capture
    #[test]
    fn test_capture() {
        let mut board = ChessBoard::new();
        board.new_game();
        board.make_move(12, 28); // e4
        board.make_move(51, 35); // d5
        let mut moves = generate_all_psuedo_legal_moves(&board, pieces::COLOR_WHITE);
        retain_only_legal_moves(&mut board, &mut moves);
        let mut captures = 0;
        for m in moves.iter() {
            if m.captured_piece.is_some() {
                captures += 1;
            }
        }
        assert_eq!(captures, 1);
    }

}