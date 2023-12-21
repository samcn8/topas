//! This module contains functions related to game state evaluation.

use crate::{chess_board, bitboard, pieces, movegen};

// Bonuses and penalities, in centipawns, for various situations
const TEMPO_BONUS: i32 = 28;
const BISHOP_PAIR_BONUS: i32 = 25;
const MOBILITY_BONUS: i32 = 10;
const ISOLATED_PAWN_PENALTY: i32 = 50;
const DOUBLE_PAWN_PENALTY: i32 = 50;
const NO_CASTLING_RIGHTS_PENALTY: i32 = 50;

// Check if the current Zobrist hash has been repeated twice before.
// Note the hash will only be the same if its the same player's turn,
// so we can skip every other element in the list
pub fn is_draw_by_threefold_repitition(board: &chess_board::ChessBoard) -> bool {
    let hash = board.zobrist_hash;
    let mut appearances = 0;
    let mut check = true;
    for h in board.zobrist_history.iter().rev() {
        if check && hash == *h {
            // Note that the first iteration will always be an "appearance"
            appearances += 1;
        }
        if appearances == 3 {
            return true;
        }
        check = !check;
    }
    false
}

// Check if there is not enough material to play on.
pub fn is_draw_by_insufficient_material(board: &chess_board::ChessBoard) -> bool {

    // If there are any pawns, rooks, or queens, it is not a draw
    if bitboard::pop_count(board.bb_pieces[pieces::COLOR_WHITE][pieces::PAWN] | board.bb_pieces[pieces::COLOR_BLACK][pieces::PAWN]) > 0 {
        return false;
    }
    if bitboard::pop_count(board.bb_pieces[pieces::COLOR_WHITE][pieces::ROOK] | board.bb_pieces[pieces::COLOR_BLACK][pieces::ROOK]) > 0 {
        return false;
    }
    if bitboard::pop_count(board.bb_pieces[pieces::COLOR_WHITE][pieces::QUEEN] | board.bb_pieces[pieces::COLOR_BLACK][pieces::QUEEN]) > 0 {
        return false;
    }

    // If the total of white's knights + bishops count is more than 1, it's not a draw
    if bitboard::pop_count(board.bb_pieces[pieces::COLOR_WHITE][pieces::KNIGHT] | board.bb_pieces[pieces::COLOR_WHITE][pieces::BISHOP]) > 1 {
        return false;
    }

    // If the total of black's knights + bishops count is more than 1, it's not a draw
    if bitboard::pop_count(board.bb_pieces[pieces::COLOR_BLACK][pieces::KNIGHT] | board.bb_pieces[pieces::COLOR_BLACK][pieces::BISHOP]) > 1 {
        return false;
    }

    // Every other case is a draw
    true

}

// Returns the game phase for tapered evaluation.
// See https://www.chessprogramming.org/Tapered_Eval
fn get_phase(board: &chess_board::ChessBoard) -> i32 {
    let knight_phase = 1;
    let bishop_phase = 1;
    let rook_phase = 2;
    let queen_phase = 4;
    let total_phase = 24; // 4*knight_phase + 4*bishop_phase + 4*rook_phase + 2*queen_phase
    let mut phase: i32 = total_phase;
    for color in 0..2 {
        phase -= (bitboard::pop_count(board.bb_pieces[color][pieces::KNIGHT]) * knight_phase) as i32;
        phase -= (bitboard::pop_count(board.bb_pieces[color][pieces::BISHOP]) * bishop_phase) as i32;
        phase -= (bitboard::pop_count(board.bb_pieces[color][pieces::ROOK]) * rook_phase) as i32;
        phase -= (bitboard::pop_count(board.bb_pieces[color][pieces::QUEEN]) * queen_phase) as i32;
    }
    (phase * 256 + (total_phase / 2)) / total_phase
}

// Returns the game board evaluation, specific to whether this is an end
// game or not, from the point of view of the
// player whose turn it is.  Returned value is in centipawns.
// Note that this assumes that the game is not over.
pub fn static_evaluation_phase(board: &chess_board::ChessBoard, is_end_game: bool) -> i32 {
    
    // Running totals of white and black evaluation
    let mut totals: [i32; 2] = [0; 2];

    // Add a tempo bonus for current player if not in the end game
    if !is_end_game {
        if board.whites_turn {
            totals[pieces::COLOR_WHITE] += TEMPO_BONUS;
        } else {
            totals[pieces::COLOR_BLACK] += TEMPO_BONUS;
        }
    }

    // Material evaluation, which is the sum of the piece value and its PST
    for color in 0..2 {
        for (piece, bb) in board.bb_pieces[color].iter().enumerate() {
            for square in bitboard::occupied_squares(*bb) {
                if is_end_game {
                    totals[color] += pieces::PIECE_VALUES[piece] + pieces::PST_END_GAME[piece][square];
                } else {
                    totals[color] += pieces::PIECE_VALUES[piece] + pieces::PST_MIDDLE_GAME[piece][square];
                }
            }
        }
    }

    // Bishop pair bonus
    if bitboard::pop_count(board.bb_pieces[pieces::COLOR_WHITE][pieces::BISHOP]) >= 2 {
        totals[pieces::COLOR_WHITE] += BISHOP_PAIR_BONUS;
    }
    if bitboard::pop_count(board.bb_pieces[pieces::COLOR_BLACK][pieces::BISHOP]) >= 2 {
        totals[pieces::COLOR_BLACK] += BISHOP_PAIR_BONUS;
    }

    // Mobility bonus, giving a bonus for every psuedo-legal move possible.
    // Note that we do not validate these moves in order to keep this function
    // as fast as possible.
    totals[pieces::COLOR_WHITE] += MOBILITY_BONUS * movegen::generate_all_psuedo_legal_moves(board, pieces::COLOR_WHITE).len() as i32;
    totals[pieces::COLOR_BLACK] += MOBILITY_BONUS * movegen::generate_all_psuedo_legal_moves(board, pieces::COLOR_BLACK).len() as i32;

    // Pawn structure penalties and bonuses
    for color in 0..2 {
        for file in 0..7 {
            let pawns_in_file = bitboard::pop_count(board.bb_pieces[color][pieces::PAWN] & bitboard::BB_FILES[file]) as i32;
            let mut neighbor_files_bb: u64 = 0;
            if file > 0 {
                neighbor_files_bb |= bitboard::BB_FILES[file-1];
            }
            if file < 7 {
                neighbor_files_bb |= bitboard::BB_FILES[file+1];
            }

            // Isolated pawn penalty
            if neighbor_files_bb & board.bb_pieces[color][pieces::PAWN] == 0 {
                totals[color] -= ISOLATED_PAWN_PENALTY * pawns_in_file;
            }

            // Double pawn penalty
            if pawns_in_file > 1 {
                totals[color] -= DOUBLE_PAWN_PENALTY * (pawns_in_file - 1);
            }

        }
    }

    // Lack of castling rights penalty
    if !board.white_castled && !board.white_ks_castling_rights && !board.white_qs_castling_rights {
        totals[pieces::COLOR_WHITE] -= NO_CASTLING_RIGHTS_PENALTY;
    }
    if !board.black_castled && !board.black_ks_castling_rights && !board.black_qs_castling_rights {
        totals[pieces::COLOR_BLACK] -= NO_CASTLING_RIGHTS_PENALTY;
    }

    // Return evaluation from the current player's perspective
    if board.whites_turn {
        totals[pieces::COLOR_WHITE] - totals[pieces::COLOR_BLACK]
    } else {
        totals[pieces::COLOR_BLACK] - totals[pieces::COLOR_WHITE]
    }
}

// Returns the phased game board evaluation from the point of view of the
// player whose turn it is.  Returned value is in centipawns.
pub fn static_evaluation(board: &chess_board::ChessBoard) -> i32 {
    let middle_game_eval = static_evaluation_phase(board, false);
    let end_game_eval = static_evaluation_phase(board, true);
    let phase = get_phase(board);
    ((middle_game_eval * (256 - phase)) + (end_game_eval * phase)) / 256
}