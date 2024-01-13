# Topas Chess by Sam Nelson

Topas Chess is an open source UCI chess engine written in Rust with a focus on clean, readable code.

Benchmarking against Elo-limited games with Stockfish, Topas Chess is estimated to have an Elo between 2250 and 2300.

## Overview

Topas Chess is a "from scratch" implementation I created as a hobby project with the goal of learning more about chess algorithms and Rust programming in general.

Note that Topas Chess is a command line program and does not contain a GUI; the only way to interact with it is via the Universal Chess Interface (UCI) protocol.  It is recommended that users use their favorite UCI-speaking chess GUI to play against Topas Chess.

The following features are implemented:
 * Universal Chess Interface (UCI) support, described below
 * Chess board representation and fast attack/movement patterns using bitboards
 * Negamax with alpha-beta pruning to efficiently search to a configurable depth
 * Iterative deepening to allow for more efficient move ordering and time management
 * Transposition tables (with Zobrist hashing) for fast lookup and enhanced move ordering
 * Move ordering based on principal variation, cut nodes, capture moves sorted via MVV-LVA (Most Valuable Victim, Least Valuable Attacker), and killer moves
 * Quiescence search with delta pruning and static exchange evaluation (SEE) to mitigate the horizon effect
 * Tapered static evaluation based on piece values, piece-square tables (PST), and game state

Topas Chess is named after "Topas", one of my children's hermit crabs.  Topas escaped in the house one day and we spent quite a few hours searching for her (successfully).  Hopefully Topas Chess will search as diligently as we did... although perhaps a bit quicker.

## UCI Support

The following UCI commands are supported:
 * `uci`: Tell the engine to use UCI mode.  Response will be `uciok`.
 * `isready`: Asks the engine if it is ready to process more commands.  Response will be `readyok`.
 * `ucinewgame`: Tell the engine that a new game is starting.  This should be sent before a `position` command if a new game is starting, so the engine can clear or reset any stored state.  There is no response to this command.
 * `position`.  Used like `position [fen <fenstring> | startpos ]  moves <move1> .... <movei>`: Tell the engine to set up the position described in `fenstring`, or set up the starting position if `startpos` is provided.  Then play the moves given in long algebraic notation.  There is no response to this command.
 * `go`.  Tell the engine to start calculated on the position provided by `position`.  Supported parameters include `depth` (maximum search depth), `wtime` and `btime` (white and black time remaining in milliseconds), and `winc` and `binc` (white and black time increments in milliseconds per the time controls of the game).  For example, `go depth 7 wtime 169604 winc 3000 btime 182062 binc 3000` tells the engine to search with a max depth of 7, considering that white has ~170 seconds left and black as ~182 seconds left, and both players have time increments of 3 seconds per move.  Response will be `bestmove <move>` when the search is over.  For example, `bestmove g5h4` indicates that the engine believes g5h4 is the best move.  Note that while the engine is searching it may send `info` messages.  For example, `info depth 3 score cp 104 nodes 2187 time 12 pv d1e1 a8d8 b1c3` is a status message indicating that the engine has just seached to depth 3, searching 2187 positions in 12 milliseconds, believes that the current player is winning by 104 centipawns, and believes the principal variation (best continuation) is d1e1 a8d8 b1c3.  Status messages do not indicate that the engine is done searching, only that it has a status update to send.
 * `quit`: Quits the program as soon as possible.
 * `print` (custom, non-UCI message): Tells the engine to print the state of the board to the screen.

## Building

To build Topas Chess, you need Rust.  Instructions for installing Rust (with Cargo) are found here: https://www.rust-lang.org/learn/get-started.

After Rust is installed, you can build using `cargo` like this:

```
cargo build --release
```

Note that it is important to build with the `--release` flag, which will signficantly improve the performance of the engine.

## Contributing

Since this is just a personal hobby project, I'm not currently accepting pull requests.  However, you are free to use the code in your own engine development in accordance with the [GNU General Public License version 3](LICENSE) (GPL v3).

## Acknowledgements

Throughout the course of this project, I read a lot of information related to searching, evaluation, and other chess-related algorithms.  A few of the key resources that helped me understand core concepts the most were:
 * The Chess Programming Wiki: https://www.chessprogramming.org/Main_Page
 * Wikipedia (which has great pseudo-code examples): https://www.wikipedia.org
 * The Mediocre Chess blog: http://mediocrechess.blogspot.com
 * Stockfish: https://github.com/official-stockfish/Stockfish
 * Snakefish (particularly the excellent explaination of Kindergarten bitboards): https://github.com/cglouch/snakefish
 * Piece square tables and piece values are from https://www.chessprogramming.org/PeSTO%27s_Evaluation_Function, which credits Ronald Friedrich's RofChade engine and specifically this forum thread: http://www.talkchess.com/forum3/viewtopic.php?f=2&t=68311&start=19.

