// This module implements the Universal Chess Interface (UCI).
// This interface uses standard input and output to interact
// with the chess engine.  There is a main processing loop
// "main_loop" which will handle all input.  Output is printed
// to standard out by the module that has relavant UCU information
// (for instance, the search module).
// See https://en.wikipedia.org/wiki/Universal_Chess_Interface

use std::io;
use std::thread;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc;
use crate::search;
use crate::pieces;
use crate::chess_board;
use crate::uci;

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