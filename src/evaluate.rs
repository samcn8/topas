//! This module contains functions related to game state evaluation.

use crate::chess_board;

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