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
    let mut engine = search::SearchEngine::new();
    engine.new_game();
    let mut board = chess_board::ChessBoard::new();
    board.new_game();
    let mut guess = String::new();
    loop {

        // Human turn
        board.print();
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

        // Computers turn
        board.print();
        println!("Computer is thinking");
        let computer_move = engine.find_best_move(5);
        if let Some(e) = computer_move {
            println!("Best move -> {:?}", e.best_move_from_last_iteration);
        } else {
            panic!("No computer move found");
        }

    }
}
