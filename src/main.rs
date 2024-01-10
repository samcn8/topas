mod chess_board;
mod zobrist;
mod pieces;
mod bitboard;
mod movegen;
mod evaluate;
mod search;
mod uci;

fn main() {
    println!("Welcome to Topas Chess by Sam Nelson!");
    let mut uci_main = uci::UCI::new();
    uci_main.main_loop();
}
