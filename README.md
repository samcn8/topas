# Topas Chess Engine by Sam Nelson

Topas is an open source UCI chess engine written in Rust with a focus on clean, readable code.

## Overview

Topas is a "from scratch" implementation I created as a hobby project with the goal of learning more about chess algorithms and Rust programming in general.

Note that Topas is a command line program and does not contain a GUI; the only way to interact with it is via the Universal Chess Interface (UCI) protocol.  It is recommended that users use their favorite UCI-speaking chess GUI to play against Topas.

The following features are implemented:
 * Universal Chess Interface (UCI) support, described below
 * Chess board representation and fast attack/movement patterns using bitboards
 * Negamax with alpha-beta pruning to efficiently search to a configurable depth
 * Iterative deepening to allow for more efficient move ordering and time management
 * Transposition tables (with Zobrist hashing) for fast lookup and enhanced move ordering
 * Move ordering based on principal variation, cut nodes, capture moves sorted via MVV-LVA (Most Valuable Victim, Least Valuable Attacker), and killer moves
 * Quiescence search with delta pruning and static exchange evaluation (SEE) to mitigate the horizon effect
 * Tapered static evaluation based on piece values, piece-square tables (PST), and game state

Topas is named after one of my children's hermit crabs.  Topas (the hermit crab) escaped in the house one day and we spent quite a few hours searching for her (successfully).  Hopefully Topas (the chess engine) will search as diligently as we did, although perhaps a bit quicker.

## Universal Chess Interface (UCI) Support

UCI dictates the use of standard input and standard output to communicate with the chess engine.  The following UCI commands are supported:

 * `uci`: Tell the engine to use UCI mode.
    * Response will provide the program name and author, and any options available.  For `topas`, this will be:
        ```
        id name Topas <version>
        id author Sam Nelson
        option name Hash type spin default 16 min 1 max 131072
        uciok
        ```
 * `setoption`: Sets engine options.
    * The only currently available option is the size of the hash table in MB.  The larger the hash table, the better `topas` will perform.  This should be sized relative to the available memory on your machine.  The UCI protocol indicates that default value should be low, which is why the default is 16MB even though modern computers would likely have significantly more memory available.
    * Usage `setoption name Hash value <value>` where value must be an integer between 1 and 131072.
    * There is no response to this command.
 * `isready`: Asks the engine if it is ready to process more commands.
    * Response will be `readyok`.
 * `ucinewgame`: Tell the engine that a new game is starting.
    * This should be sent before a `position` command if a new game is starting, so the engine can clear or reset any stored state.
    * There is no response to this command.
 * `position`: Set the board position.
    * Usage: `position [fen <fenstring> | startpos ]  moves <move1> .... <movei>`.  Tell the engine to set up the position described in `fenstring`, or set up the starting position if `startpos` is provided.  Then play the moves given in long algebraic notation.
    * There is no response to this command.
 * `go`: Tell the engine to start calculating on the position provided by `position`.
    * The following are supported parameters to the `go` command:
       * `depth`: Maximum depth the engine should search
       * `wtime`: White's remaining time in milliseconds until the next time controls (or, if sudden death, for the game)
       * `btime`: Black's remaining time in milliseconds until the next time controls (or, if sudden death, for the game)
       * `winc`: White's increment per the time controls of the game
       * `binc`: Black's increment per the time controls of the game
       * `movestogo`: Number of moves remaining until the next time control.  Note that if this parameter is set, it must be greater than 0.  If the parameter is not set, it is assumed to be sudden death (meaning the remaining time is for the entire game).
       * `movetime`: Search for exactly the specified number of milliseconds.
       * `infinite`: Search until the `stop` command is received
    * Response will be `bestmove <move>` when the search is over.  For example, `bestmove g5h4` indicates that the engine believes g5h4 is the best move.
    * While the engine is searching, it may send `info` messages.  For example, `info depth 3 score cp 104 nodes 2187 time 12 pv d1e1 a8d8 b1c3` is a status message indicating that the engine has just searched to depth 3, searching 2187 positions in 12 milliseconds, believes that the current player is winning by 104 centipawns, and believes the principal variation (best continuation) is d1e1 a8d8 b1c3.  Status messages do not indicate that the engine is done searching, only that it has a status update to send.
 * `stop`: If actively searching, stop searching as soon as possible and return the best move.
 * `quit`: Quits the program as soon as possible.
 * `print` (custom, non-UCI message): Tells the engine to print the state of the board to the screen.

Here is an example of Topas searching, at depth 8, for the best move from the starting position (added `>` characters to indicate user input for clarity):

```
$ ./topas
Topas 1.2.0 by Sam Nelson
> uci
id name Topas 1.2.0
id author Sam Nelson
option name Hash type spin default 16 min 1 max 131072
uciok
> setoption name Hash value 1000
> ucinewgame
> isready
readyok
> position startpos
> go depth 8
info depth 1 score cp 8 nodes 21 time 0 pv g1f3 
info depth 2 score cp 28 nodes 79 time 0 pv g1f3 g8f6 
info depth 3 score cp 7 nodes 708 time 2 pv g1f3 g8f6 d2d4 
info depth 4 score cp 28 nodes 1697 time 8 pv g1f3 g8f6 d2d4 d7d5 
info depth 5 score cp 5 nodes 14934 time 48 pv g1f3 g8f6 d2d4 d7d5 b1c3 
info depth 6 score cp 28 nodes 54147 time 219 pv g1f3 g8f6 d2d4 d7d5 b1c3 b8c6 
info depth 7 score cp 3 nodes 353349 time 1209 pv g1f3 g8f6 e2e3 d7d5 c2c4 c8e6 c4d5 
info depth 8 score cp 34 nodes 1534560 time 6191 pv e2e4 d7d5 e4d5 g8f6 d2d4 f6d5 c2c4 d5b4 
bestmove e2e4 
```

## Building

To build Topas, you need Rust.  Instructions for installing Rust (with Cargo) are found here: https://www.rust-lang.org/learn/get-started.

After Rust is installed, you can build using `cargo` like this:

```
cargo build --release
```

Note that it is important to build with the `--release` flag, which will significantly improve the performance of the engine.

The resulting executable can be found in:

```
target/release/
```

## Contributing

Since this is just a personal hobby project, I'm not currently accepting pull requests.  However, you are free to use the code in your own engine development in accordance with the [GNU General Public License version 3](LICENSE) (GPL v3).

## Acknowledgments

Throughout the course of this project, I read a lot of information related to searching, evaluation, and other chess-related algorithms.  A few of the key resources that helped me understand core concepts the most were:
 * The Chess Programming Wiki: https://www.chessprogramming.org/Main_Page
 * Wikipedia (which has great pseudo-code examples): https://www.wikipedia.org
 * The Mediocre Chess blog: http://mediocrechess.blogspot.com
 * Stockfish: https://github.com/official-stockfish/Stockfish
 * Snakefish (particularly the excellent explanation of Kindergarten bitboards): https://github.com/cglouch/snakefish
 * Piece square tables and piece values are from https://www.chessprogramming.org/PeSTO%27s_Evaluation_Function, which credits Ronald Friedrich's RofChade engine and specifically this forum thread: http://www.talkchess.com/forum3/viewtopic.php?f=2&t=68311&start=19.

