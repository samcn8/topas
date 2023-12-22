//! This module contains information and helpful constants related
//! to game pieces and scoring.

// Constants for identifying pieces
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

// Constants for centipawn value of pieces (indexes using
// the piece constants listed above)
pub const PIECE_VALUES: [i32; 6] = [100, 320, 330, 500, 900, 20000];

// Most valuable victom / least valuable attacker (MVV-LVA).  This is used
// for ordering capture moves.  Higher numbers result in higher
// priority for move ordering.
// See https://www.chessprogramming.org/MVV-LVA
pub const MVV_LVA: [[i32; 6]; 5] = [
    [5, 4, 3, 2, 1, 0],  // Pawn Victim -> PNBRQK Attackers
    [11, 10, 9, 8, 7, 6], // Knight Victim -> PNBRQK Attackers
    [17, 16, 15, 14, 13, 12], // Bishop Victim -> PNBRQK Attackers
    [23, 22, 21, 20, 19, 18], // Room Victim -> PNBRQK Attackers
    [29, 28, 27, 26, 25, 24], // Queen Victim -> PNBRQK Attackers
];

// Piece square tables (PST) for augmenting piece values
// based on where they reside.
// See https://www.chessprogramming.org/Simplified_Evaluation_Function
const fn create_pst_pawn() -> [i32; 64] {
    let pst: [i32; 64] =
        [0,  0,  0,  0,  0,  0,  0,  0,
         50, 50, 50, 50, 50, 50, 50, 50,
         10, 10, 20, 30, 30, 20, 10, 10,
         5,  5, 10, 25, 25, 10,  5,  5,
         0,  0,  0, 20, 20,  0,  0,  0,
         5, -5,-10,  0,  0,-10, -5,  5,
         5, 10, 10,-20,-20, 10, 10,  5,
         0,  0,  0,  0,  0,  0,  0,  0];
    order_pst(pst)
}
const fn create_pst_knight() -> [i32; 64] {
    let pst: [i32; 64] =
        [-50,-40,-30,-30,-30,-30,-40,-50,
         -40,-20,  0,  0,  0,  0,-20,-40,
         -30,  0, 10, 15, 15, 10,  0,-30,
         -30,  5, 15, 20, 20, 15,  5,-30,
         -30,  0, 15, 20, 20, 15,  0,-30,
         -30,  5, 10, 15, 15, 10,  5,-30,
         -40,-20,  0,  5,  5,  0,-20,-40,
         -50,-40,-30,-30,-30,-30,-40,-50];
    order_pst(pst)
}
const fn create_pst_bishop() -> [i32; 64] {
    let pst: [i32; 64] =
        [-20,-10,-10,-10,-10,-10,-10,-20,
         -10,  0,  0,  0,  0,  0,  0,-10,
         -10,  0,  5, 10, 10,  5,  0,-10,
         -10,  5,  5, 10, 10,  5,  5,-10,
         -10,  0, 10, 10, 10, 10,  0,-10,
         -10, 10, 10, 10, 10, 10, 10,-10,
         -10,  5,  0,  0,  0,  0,  5,-10,
         -20,-10,-10,-10,-10,-10,-10,-20];
    order_pst(pst)
}
const fn create_pst_rook() -> [i32; 64] {
    let pst: [i32; 64] =
        [0,  0,  0,  0,  0,  0,  0,  0,
         5, 10, 10, 10, 10, 10, 10,  5,
        -5,  0,  0,  0,  0,  0,  0, -5,
        -5,  0,  0,  0,  0,  0,  0, -5,
        -5,  0,  0,  0,  0,  0,  0, -5,
        -5,  0,  0,  0,  0,  0,  0, -5,
        -5,  0,  0,  0,  0,  0,  0, -5,
         0,  0,  0,  5,  5,  0,  0,  0];
    order_pst(pst)
}
const fn create_pst_queen() -> [i32; 64] {
    let pst: [i32; 64] =
        [-20,-10,-10, -5, -5,-10,-10,-20,
         -10,  0,  0,  0,  0,  0,  0,-10,
         -10,  0,  5,  5,  5,  5,  0,-10,
          -5,  0,  5,  5,  5,  5,  0, -5,
           0,  0,  5,  5,  5,  5,  0, -5,
         -10,  5,  5,  5,  5,  5,  0,-10,
         -10,  0,  5,  0,  0,  0,  0,-10,
         -20,-10,-10, -5, -5,-10,-10,-20];
    order_pst(pst)
}
const fn create_pst_king_mg() -> [i32; 64] {
    let pst: [i32; 64] =
        [-30,-40,-40,-50,-50,-40,-40,-30,
         -30,-40,-40,-50,-50,-40,-40,-30,
         -30,-40,-40,-50,-50,-40,-40,-30,
         -30,-40,-40,-50,-50,-40,-40,-30,
         -20,-30,-30,-40,-40,-30,-30,-20,
         -10,-20,-20,-20,-20,-20,-20,-10,
          20, 20,  0,  0,  0,  0, 20, 20,
          20, 30, 10,  0,  0, 10, 30, 20];
    order_pst(pst)
}
const fn create_pst_king_eg() -> [i32; 64] {
    let pst: [i32; 64] =
        [-50,-40,-30,-20,-20,-30,-40,-50,
         -30,-20,-10,  0,  0,-10,-20,-30,
         -30,-10, 20, 30, 30, 20,-10,-30,
         -30,-10, 30, 40, 40, 30,-10,-30,
         -30,-10, 30, 40, 40, 30,-10,-30,
         -30,-10, 20, 30, 30, 20,-10,-30,
         -30,-30,  0,  0,  0,  0,-30,-30,
         -50,-30,-30,-30,-30,-30,-30,-50];
    order_pst(pst)
}
pub const PST_MIDDLE_GAME: [[i32; 64]; 6] = [
    create_pst_pawn(),
    create_pst_knight(),
    create_pst_bishop(),
    create_pst_rook(),
    create_pst_queen(),
    create_pst_king_mg(),
];
pub const PST_END_GAME: [[i32; 64]; 6] = [
    create_pst_pawn(),
    create_pst_knight(),
    create_pst_bishop(),
    create_pst_rook(),
    create_pst_queen(),
    create_pst_king_eg(),
];

// Re-order values in PST to go from a "human readable" chess board
// visualization to a proper square ID order
const fn order_pst(pst: [i32; 64]) -> [i32; 64] {
    let mut ordered_pst: [i32; 64] = [0; 64];
    let mut square: usize = 0;
    loop {
        let row = 7 - (square / 8);
        let col = square % 8;
        ordered_pst[square] = pst[row * 8 + col];
        square += 1;
        if square >= 64 {
            break;
        }
    }
    ordered_pst
}