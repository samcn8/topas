//! Welcome to Topas by Sam Nelson!
//! 
//! This is the entry point into the Topas engine.  Control is
//! immediately passed to the Universal Chess Interface (UCI)
//! handling loop.

mod chess_board;
mod zobrist;
mod pieces;
mod bitboard;
mod movegen;
mod evaluate;
mod search;
mod uci;

fn main() {
    println!("Topas 0.1.0 by Sam Nelson");
    let mut uci_main = uci::UCI::new();
    uci_main.main_loop();
}
