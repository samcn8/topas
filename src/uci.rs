// This module implements the Universal Chess Interface (UCI).

use std::io;
use crate::search;

pub struct UCI {
    
    // The engine
    engine: search::SearchEngine,

}

impl UCI {

    // Construct a new ChessBoard
    pub fn new() -> UCI {
        UCI {
            engine: search::SearchEngine::new(),
        }
    }

    // The main UCI processing loop
    pub fn main_loop(&mut self) {

        loop {

            // Get the UCI command and parse into tokens
            let mut uci_command_raw = String::new();
            io::stdin().read_line(&mut uci_command_raw).expect("Failed to read line");
            let mut uci_command = uci_command_raw.to_lowercase();
            let tokens: Vec<&str> = uci_command.split_whitespace().collect();
            
            // Process the command based on the first token
            if !tokens.is_empty() {
                match tokens[0] {
                    "uci" => self.uci_command(),
                    "isready" => self.isready_command(),
                    "ucinewgame" => self.ucinewgame_command(),
                    "position" => self.position_command(&tokens),
                    "go" => self.go_command(&tokens),
                    "quit" => break,
                    _ => self.unknown_command(),
                }
            }
            
            // Prepare for next command
            uci_command.clear();

        }

    }

    // Process the "uci" command
    fn uci_command(&self) {
        println!("id name Topas Chess");
        println!("id author Sam Nelson");
        println!("uciok");
    }

    // Process the "isready" command
    fn isready_command(&self) {
        println!("readyok");
    }

    // Process the "ucinewgame" command
    fn ucinewgame_command(&mut self) {
        self.engine.new_game();
    }

    // Process the "position" command
    // Note that if this is a new game, then the "ucinewgame" command should
    // have been sent before this, which clears the transposition tables.
    fn position_command(&mut self, tokens: &Vec<&str>) {
        if tokens.len() >= 2 {
            if tokens[1] == "startpos" {
                let mut move_str = String::new();
                for i in 3..tokens.len() {
                    move_str.push_str(tokens[i]);
                    move_str.push_str(" ");
                }
                self.engine.set_board_state(&move_str);
            }
        }
    }

    // Process the "go" command
    // This is the main request to search.  The search must run
    // in a seperate thread in order to keep UCI responsive.
    fn go_command(&mut self, tokens: &Vec<&str>) {
        if tokens.len() == 3 && tokens[1] == "depth" {
            if let Ok(d) = tokens[2].parse::<u8>() {
                // TODO -- this needs to be handled in a sepearte thread
                self.engine.find_best_move(d).unwrap();
            }
        }
    }

    // Process an unknown commabd
    fn unknown_command(&self) {
        println!("Unknown command");
    }

}

// For debugging, allow the user to play a game on the console.
/*
fn play_computer() {

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
*/