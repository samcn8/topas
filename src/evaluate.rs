//! This module contains functions related to game state evaluation.
//! Evaluation is typically done in centipawns (1/100 of a pawn).
//! This primary performs a static evaluation, meaning the evaluation
//! is done based on the current state of the board without any additional
//! searching.

use crate::chess_board;
use crate::bitboard;
use crate::pieces;

// Bonuses and penalities, in centipawns, for various situations
const TEMPO_BONUS: i32 = 28;
const BISHOP_PAIR_BONUS: i32 = 25;
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
                
                // The PST's are from white's perspective, so we have to flip
                // the look up for black.  Performing a bitwise "xor 56" on
                // the square will "flip" the square to the other side.
                if is_end_game {
                    if color == pieces::COLOR_WHITE {
                        totals[color] += pieces::PIECE_VALUES_EG[piece] + pieces::PST_END_GAME[piece][square];
                    } else {
                        totals[color] += pieces::PIECE_VALUES_EG[piece] + pieces::PST_END_GAME[piece][square ^ 56];
                    }
                } else {
                    if color == pieces::COLOR_WHITE {
                        totals[color] += pieces::PIECE_VALUES_MG[piece] + pieces::PST_MIDDLE_GAME[piece][square];
                    } else {
                        totals[color] += pieces::PIECE_VALUES_MG[piece] + pieces::PST_MIDDLE_GAME[piece][square ^ 56];
                    }
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

    // Passed pawn bonuses based on rank.  Bonuses are 16*row where row is 1
    // for the starting position (regardless of color).
    for color in 0..2 {
        for square in bitboard::occupied_squares(board.bb_pieces[color][pieces::PAWN]) {
            if bitboard::BB_PAWN_FRONT_SPAN[color][square] & board.bb_pieces[1-color][pieces::PAWN] == 0 {
                // This is a passed pawn
                let row = if color == pieces::COLOR_WHITE {square / 8} else {7 - (square / 8)};
                totals[color] += (16 * row) as i32;
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

// Returns the game phase for tapered evaluation.  This blends the middle game
// and end game evaluation as pieces are removed to avoid a dramatic shift
// in evaluation between the middle and end game.
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