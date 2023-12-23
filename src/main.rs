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
    let mut move_string = String::new();
    loop {

        // Human turn
        board.print();
        println!("Enter move in long algebraic notation: ");
        io::stdin()
            .read_line(&mut guess)
            .expect("Failed to read line");
        let trimmed_move = guess.trim();
        let cur_move = movegen::convert_moves_str_into_list(trimmed_move);
        board.make_move(cur_move[0].0, cur_move[0].1);
        move_string.push_str(&trimmed_move);
        move_string.push_str(" ");
        guess.clear();
        println!("Move string so far: {}", move_string);

        // Computers turn
        engine.set_board_state(&move_string);
        board.print();
        println!("Computer is thinking");
        let computer_move = engine.find_best_move(7).unwrap();
        println!("Best move -> {:?}", computer_move.best_move_from_last_iteration);
        let c_move = computer_move.best_move_from_last_iteration.unwrap();
        board.make_move(c_move.0 as usize, c_move.1 as usize);

        // Convert the move into long algebraic notation
        let rank_start = (c_move.0 / 8 + 1).to_string();
        let rank_end = (c_move.1 / 8 + 1).to_string();
        let file_start = "abcdefgh".chars().nth((c_move.0 % 8) as usize).unwrap().to_string();
        let file_end= "abcdefgh".chars().nth((c_move.1 % 8) as usize).unwrap().to_string();
        move_string.push_str(&file_start);
        move_string.push_str(&rank_start);
        move_string.push_str(&file_end);
        move_string.push_str(&rank_end);
        move_string.push_str(" ");
        println!("Move string so far: {}", move_string);


    }
}
