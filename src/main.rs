mod chess_board;
mod zobrist;
mod pieces;
mod bitboard;
mod movegen;
mod evaluate;
mod search;

use std::io;

fn main() {
    println!("Welcome to Topas Chess by Sam Nelson!");
    let mut board = chess_board::ChessBoard::new();
    board.new_game();
    let mut guess = String::new();
    loop {
        board.print();
        let moves = movegen::generate_all_psuedo_legal_moves(&board);
        println!("{:#?}", moves);
        println!("Enter starting square of move or u to undo: ");
        io::stdin()
            .read_line(&mut guess)
            .expect("Failed to read line");
        if guess.trim() == "u" {
            guess.clear();
            board.unmake_move();
            continue;
        }
        let start_square: usize = guess.trim().parse().expect("Please type a number!");
        guess.clear();
        println!("Enter ending square of move: ");
        io::stdin()
            .read_line(&mut guess)
            .expect("Failed to read line");
        let end_square: usize = guess.trim().parse().expect("Please type a number!");
        guess.clear();
        board.make_move(start_square, end_square);
    }
}
