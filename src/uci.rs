// This module implements the Universal Chess Interface (UCI).
// This interface uses standard input and output to interact
// with the chess engine.  There is a main processing loop
// "main_loop" which will handle all input.  Output is printed
// to standard out by the module that has relavant UCU information
// (for instance, the search module).
// See https://en.wikipedia.org/wiki/Universal_Chess_Interface

use std::io;
use std::io::Write;
use std::thread;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc;
use crate::search;
use crate::pieces;
use crate::chess_board;
use crate::uci;
use crate::movegen;
use crate::evaluate;

pub struct UCI {
    
    // Long lived thread that the engine will run in
    engine_thread: Option<thread::JoinHandle<()>>,

    // Transmission channel to send commands to the engine thread
    tx: Sender<String>,

}

impl UCI {

    // Construct a new engine in a seperate thread.  Communication with
    // this thread will be done via Channels.
    pub fn new() -> UCI {

        // Communication channel between the main thread and the engine
        // thread.  All communication is from the main thread (tx) to the
        // engine thread (rx).
        let (tx, rx): (Sender<String>, Receiver<String>) = mpsc::channel();

        // Spawn a long lived thread that will handle engine execution.
        let t = Some(thread::spawn(move || {

            let mut engine = search::SearchEngine::new(rx);
            engine.new_game();
            loop {

                // Wait on a command (note this is a blocking call)
                let uci_command = engine.rx_channel.recv().unwrap();

                // Parse and act on the UCI command.  Note that the stop
                // command is handled by the engine, which will periodically
                // check the receiver for that command.
                let tokens: Vec<&str> = uci_command.split_whitespace().collect();
                if !tokens.is_empty() {
                    match tokens[0] {
                        "setoption" => uci::setoption_command(&mut engine, &tokens),
                        "ucinewgame" => uci::ucinewgame_command(&mut engine),
                        "position" => uci::position_command(&mut engine, &tokens),
                        "go" => uci::go_command(&mut engine, &tokens),
                        "stop" => {},
                        "print" => uci::print_board(&mut engine),
                        "quit" => break,
                        _ => println!("Unknown command"),
                    }
                }

            }
        }));

        // return UCI state
        UCI {
            engine_thread: t,
            tx,
        }

    }

    // The main UCI processing loop
    pub fn main_loop(&mut self) {

        loop {

            // Get the UCI command and parse it into tokens
            let mut uci_command = String::new();
            io::stdin().read_line(&mut uci_command).expect("Failed to read line");
            let tokens: Vec<&str> = uci_command.split_whitespace().collect();
            
            // Process the command based on the first token
            if !tokens.is_empty() {
                match tokens[0] {
                    "uci" => uci::uci_command(),
                    "isready" => uci::isready_command(),
                    "terminal" => uci::play_terminal(),
                    "quit" => break,
                    _ => self.tx.send(uci_command).unwrap(),
                }
            }

        }

        // Send a "stop command" to interupt any current search, and then
        // send a "quit" command and wait for the engine thread to exit.
        self.tx.send(String::from("stop")).unwrap();
        self.tx.send(String::from("quit")).unwrap();
        self.engine_thread.take().map(thread::JoinHandle::join);

    }

}

// Process the "uci" command within the main thread.
pub fn uci_command() {
    println!("id name Topas {}", env!("CARGO_PKG_VERSION"));
    println!("id author Sam Nelson");
    println!("option name Hash type spin default {} min 1 max 131072", search::DEFAULT_TT_SIZE_MB);
    println!("uciok");
}

// Process the "isready" command within the main thread.
pub fn isready_command() {
    println!("readyok");
}

// Process the "setoption" command within the engine thread.
pub fn setoption_command(engine: &mut search::SearchEngine, tokens: &Vec<&str>) {
    if tokens.len() == 5 && tokens[1] == "name" && tokens[2] == "Hash" && tokens[3] == "value" {
        if let Ok(d) = tokens[4].parse::<u64>() {
            if d >= 1 && d <= 131072 {
                engine.set_tt_size_mb(d);
            } else {
                println!("Hash value out of range");
            }
        } else {
            println!("Invalid value for Hash");
        }
    } else {
        println!("Invalid option");
    }
}

// Process the "position" command within the engine thread.
// Note that if this is a new game, then the "ucinewgame" command should
// have been sent before this, which clears the transposition tables.
pub fn position_command(engine: &mut search::SearchEngine, tokens: &Vec<&str>) {
    if tokens.len() >= 2 {

        let fen_str;
        let mut move_str = String::new();
        let move_start;

        // Get the FEN representation of the board
        if tokens[1] == "startpos" {
            fen_str = String::from(chess_board::STARTFEN);
            move_start = 2;
        } else if tokens[1] == "fen" && tokens.len() >= 8 {
            fen_str = format!("{} {} {} {} {} {}", tokens[2], tokens[3], tokens[4], tokens[5], tokens[6], tokens[7]);
            move_start = 8;
        } else {
            return;
        }

        // Get any moves associated with the board
        if tokens.len() > move_start && tokens[move_start] == "moves" {
            for i in (move_start+1)..tokens.len() {
                move_str.push_str(tokens[i]);
                move_str.push_str(" ");
            }
        }

        // Set the board state
        engine.set_board_state(&fen_str, &move_str);
    }
}

// Process the "ucinewgame" command within the engine thread.
pub fn ucinewgame_command(engine: &mut search::SearchEngine) {
    engine.new_game();
}

// Process the "go" command within the engine thread.
// This is the main request to search.
pub fn go_command(engine: &mut search::SearchEngine, tokens: &Vec<&str>) {
    let my_color = engine.color_turn();
    let my_color_time_param = if my_color == pieces::COLOR_WHITE {"wtime"} else {"btime"};
    let my_color_inc_param = if my_color == pieces::COLOR_WHITE {"winc"} else {"binc"};
    let mut my_time = -1;
    let mut my_inc = -1;
    let mut depth = 0;

    // If we don't get a "movestogo" parameter, we assume it is sudden
    // death time controls.  In this case, always assume we have 25 moves
    // left in the game.
    let mut movestogo = 25;

    // Extract the requested depth, if provided
    if let Some(e) = tokens.iter().position(|&x| x == "depth") {
        if tokens.len() > e+1 {
            if let Ok(d) = tokens[e+1].parse::<u8>() {
                depth = d;
            }
        }
    }

    // Extract the time remaining and increment, if provided
    if let Some(e) = tokens.iter().position(|&x| x == my_color_time_param) {
        if tokens.len() > e+1 {
            if let Ok(d) = tokens[e+1].parse::<i32>() {
                my_time = d;
            }
        }
    }
    if let Some(e) = tokens.iter().position(|&x| x == my_color_inc_param) {
        if tokens.len() > e+1 {
            if let Ok(d) = tokens[e+1].parse::<i32>() {
                my_inc = d;
            }
        }
    }

    // Extract the moves to go until the next time control
    if let Some(e) = tokens.iter().position(|&x| x == "movestogo") {
        if tokens.len() > e+1 {
            if let Ok(d) = tokens[e+1].parse::<u16>() {
                // Note that this should not be sent with 0, but be safe
                if d != 0 {
                    movestogo = d;
                }
            }
        }
    }

    // Extract "movetime", if set.  Note that this will override any other
    // time controls.
    if let Some(e) = tokens.iter().position(|&x| x == "movetime") {
        if tokens.len() > e+1 {
            if let Ok(d) = tokens[e+1].parse::<u32>() {
                // Tell the engine to spend exactly this many milliseconds
                // TODO: The engine may choose to not start another iterative
                // deepening loop if it does not believe it can complete it
                // in time.  This option should probably override that
                // behavior, since the UCI protocol specifies that "movetime"
                // should search "exactly" the given number of milliseconds.
                // Also note that we're not modifying depth here.
                my_time = d as i32;
                my_inc = 0;
                movestogo = 1;
            }
        }
    }

    // Perform the search with either depth or time as a limiter.
    // If neither of these is present, check for a "infinite" command.
    if depth > 0 || my_time > 0  || tokens.iter().any(|&x| x == "infinite") {
        engine.find_best_move(depth, my_time, my_inc, movestogo);
    } else {
        println!("Invalid go parameters; ignoring");
    }

}

// Extra (non-UCI) print command for debuging, handled within the
// engine thread.
pub fn print_board(engine: &mut search::SearchEngine) {
    engine.print_board();
}

// Play a terminal game
pub fn play_terminal() {

    // Create a new engine and board
    let (_, rx): (Sender<String>, Receiver<String>) = mpsc::channel();
    let mut engine = search::SearchEngine::new(rx);
    engine.new_game();
    let mut board = chess_board::ChessBoard::new();

    // Get initial input
    let mut use_unicode = false;
    let human_color;
    let mut time_per_move = 5000;
    println!();
    println!("===================================");
    println!("Welcome to the Topas Chess Terminal");
    println!("===================================");
    println!();
    println!("Note: For a more feature-rich user experience, consider using a UCI-based chess GUI.");
    println!();
    println!("Default options are: ");
    println!("   - Unicode support: no");
    println!("   - Topas hash table size: 2GB");
    println!("   - Topas time per move: 5 seconds");
    let use_defaults;
    loop {
        print!("Do you want to continue with these defaults ('yes' to continue, 'no' to edit): ");
        io::stdout().flush().unwrap();
        match get_user_input().as_str() {
            "yes" | "y" => {use_defaults = true; break},
            "no" | "n" => {use_defaults = false; break},
            _ => println!(" -> Invalid input, please enter 'yes' or 'no'."),
        }
    }
    if !use_defaults {
        loop {
            print!("Does your terminal support unicode characters (yes/no) (enter yes if unsure)? ");
            io::stdout().flush().unwrap();
            match get_user_input().as_str() {
                "yes" | "y" => {use_unicode = true; break},
                "no" | "n" => {use_unicode = false; break},
                _ => println!(" -> Invalid input, please enter 'yes' or 'no'."),
            }
        }
        loop {
            print!("Enter the hash table size (in MB from 1 to 131072) that Topas is allowed to use (enter 2000 if unsure): ");
            io::stdout().flush().unwrap();
            let input = get_user_input();
            if let Ok(i) = input.parse::<u32>() {
                if i >= 1 && i <= 131072 {
                    engine.set_tt_size_mb(i as u64);
                    break;
                }
            }
            println!(" -> Invalid input, please enter an integer between 1 and 131072.");
        }
        loop {
            print!("Enter the number of milliseconds per move that Topas is allowed (enter 5000 if unsure): ");
            io::stdout().flush().unwrap();
            let input = get_user_input();
            if let Ok(i) = input.parse::<u32>() {
                if i >= 1 && i <= 1000000 {
                    time_per_move = i;
                    break;
                }
            }
            println!(" -> Invalid input, please enter an integer between 1 and 1000.");
        }
    }
    loop {
        print!("Would you like to play as white or black? ");
        io::stdout().flush().unwrap();
        match get_user_input().as_str() {
            "white" | "w" => {human_color = pieces::COLOR_WHITE; break},
            "black" | "b" => {human_color = pieces::COLOR_BLACK; break},
            _ => println!(" -> Invalid input, please enter 'white' or 'black'."),
        }
    }
    loop {
        println!();
        println!("You're all set!  Note that all moves must be entered in UCI-style long");
        println!("algebraic notation.  This means 4 characters (2 for start square, 2 for");
        println!("end square).  For instance e2e4 moves the pawn two spaces.  If you make");
        println!("a promotion move, then a 5th character should be added representing the");
        println!("new piece in lowercase.  For instance, b7b8q promotes a black pawn to a");
        print!("queen.  Got it (yes/no)? ");
        io::stdout().flush().unwrap();
        match get_user_input().as_str() {
            "yes" | "y" => break,
            _ => println!(" -> See https://en.wikipedia.org/wiki/Algebraic_notation_(chess)"),
        }
    }

    // Play the game
    let mut move_string = String::new();
    board.new_game();
    let mut turn = pieces::COLOR_WHITE;
    loop {
        let mut cur_move;
        let mut move_raw;
        println!();
        if human_color == pieces::COLOR_BLACK {
            println!("Black: You");
        } else {
            println!("Black: Topas");
        }
        board.print(use_unicode);
        if human_color == pieces::COLOR_WHITE {
            println!("White: You");
        } else {
            println!("White: Topas");
        }
        println!();
        if turn == human_color {
            loop {

                // Get move from user
                print!("Your turn - enter move in long algebraic notation (type quit to quit): ");
                io::stdout().flush().unwrap();
                move_raw = get_user_input();
                if move_raw == "quit" {
                    println!("You are leaving the Topas Chess Terminal and switching back into UCI mode.");
                    println!("Enter quit again to exit the program; else enter any other UCI command.");
                    return;
                }
                if !valid_move_entry(&move_raw) {
                    println!(" -> Invalid move input");
                    continue;
                }
                cur_move = movegen::convert_moves_str_into_list(&move_raw);
                let cur_piece = board.get_color_and_piece_on_square(cur_move[0].0);
                if cur_piece.is_none() {
                    println!(" -> Invalid move, no piece on selected square");
                    continue;
                }
                let mut cap_piece = None;
                if let Some(e) = board.get_color_and_piece_on_square(cur_move[0].1) {
                    cap_piece = Some(e.1);
                }
                let start_file = cur_move[0].0 % 8;
                let end_file = cur_move[0].1 % 8;
                let mut is_en_passant = false;
                if cur_piece.unwrap().1 == pieces::PAWN && (start_file != end_file) {
                    is_en_passant = true;
                }
                let cur_move_struct = movegen::ChessMove {
                    start_square: cur_move[0].0,
                    end_square: cur_move[0].1,
                    piece: cur_piece.unwrap().1,
                    captured_piece: cap_piece,
                    priority: 0,
                    is_en_passant,
                };

                // Validate move
                if !movegen::generate_all_psuedo_legal_moves(&board, turn, false).contains(&cur_move_struct) || !movegen::is_legal_move(&mut board, &cur_move_struct) {
                    println!(" -> Illegal move");
                    continue;
                }
                break;
            }
        } else {

            // Get best move from engine
            println!("Topas is now thinking...");
            engine.set_board_state(chess_board::STARTFEN, &move_string);
            move_raw = engine.find_best_move(99, time_per_move as i32, 0, 1);
            cur_move = movegen::convert_moves_str_into_list(&move_raw);

        }
        
        // Make the move and switch turns
        board.make_move(cur_move[0].0, cur_move[0].1, Some(pieces::QUEEN));
        move_string.push_str(&move_raw);
        move_string.push_str(" ");
        turn = 1 - turn;

        // Check for game end state
        let mut all_moves = movegen::generate_all_psuedo_legal_moves(&board, turn, false);
        all_moves.retain(|x| movegen::is_legal_move(&mut board, &x));
        if all_moves.len() == 0 {
            if movegen::is_king_in_check(&board, turn) {
                println!("Game over: {} wins by checkmate", if turn == pieces::COLOR_WHITE {"Black"} else {"White"});
            } else {
                println!("Game over: Draw by stalemate");
            }
            break;
        }
        if evaluate::is_draw_by_insufficient_material(&board) {
            println!("Game over: Draw by insufficient material");
            break;
        }
        if evaluate::is_draw_by_threefold_repitition(&board) {
            println!("Game over: Draw by threefold repitition");
            break;
        }
    }

    // Exit terminal
    println!();
    println!("You are leaving the Topas Chess Terminal and switching back into UCI mode.");
    println!("Enter quit again to exit the program; else enter any other UCI command.");

}

// Validate move string
fn valid_move_entry(m: &str) -> bool {
    if m.len() < 4 || m.len() > 5 {
        return false;
    }
    if !m.chars().nth(1).unwrap().is_digit(10) || !m.chars().nth(3).unwrap().is_digit(10) {
        return false;
    }
    if !"abcdefgh".contains(m.chars().nth(0).unwrap()) || !"abcdefgh".contains(m.chars().nth(2).unwrap()) {
        return false;
    }
    if m.len() == 5 && !"nbrq".contains(m.chars().nth(4).unwrap()) {
        return false;
    }
    true
}

// Get user input
fn get_user_input() -> String {
    let mut input = String::new();
    io::stdin().read_line(&mut input).expect("Failed to read line");
    input.trim().to_lowercase().to_string()
}