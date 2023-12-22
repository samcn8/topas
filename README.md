# Topas Chess by Sam Nelson

Topas Chess is an open source UCI chess engine written in Rust with a focus on clean, readable code.

## Overview

Topas Chess is a "from scratch" implementation I created as a hobby project with the goal of learning more about chess algorithms and Rust programming in general.  My goal was to create clean, readable code that was well-documented, as opposed to an engine that maximized performance.

Note that Topas Chess does not contain a GUI; the only way to interact with it is via the Universal Chess Interface (UCI) protocol.

The following features are implemented:
 * Universal Chess Interface (UCI) support
 * Chess board representation using bitboards
 * Negamax with alpha-beta pruning to efficiently search to a configurable depth
 * Iterative deepening to allow for more efficient move ordering and time management
 * Transposition tables (with Zobrist hashing) for fast lookup and enhanced move ordering
 * Intelligent move ordering based on principal variation, cut nodes, and capture moves sorted via MVV-LVA (Most Valuable Victim, Least Valuable Attacker)
 * Quiescence search with static exchange evaluation (SEE) and delta pruning to mitigate the horizon effect
 * Tapered static evaluation based on piece values, piece-square tables (PST), and game state

Topas Chess is named after "Topas", one of my children's hermit crabs.  Topas escaped in the house one day and we spent quite a few hours searching for her (succesfully).  Hopefully Topas Chess will search as diligently as we did... although perhaps a bit quicker.

## UCI Support

TBD

## Building

To build Topas Chess, you need the Rust compiler suite.  Instructions for installing Rust (with Cargo) are found here: https://www.rust-lang.org/learn/get-started.

After Rust is installed, you can build using `cargo` like this:

```
cargo build --release
```

Note that it is important to build with the `-release` flag, which will signficantly improve the performance of the engine.

## Contributing - Not currently accepting pull requests

This is a personal hobby project and something I enjoy tickering with in my free time.  I'm not currently accepting issues or pull requests.  However, you are free to use the code in your own engine development in accordance with the [GNU General Public License version 3](LICENSE) (GPL v3).  Enjoy!

## Acknowledgements

Throughout the course of this project, I scoured the Internet for information related to searching, evaluation, and other chess-related algorithms.  A few of the key resources that helped me understand core concepts the most were:
 * The Chess Programming Wiki: https://www.chessprogramming.org/Main_Page
 * Wikipedia (which has great pseudo-code examples): https://www.wikipedia.org
 * The Mediocre Chess blog: http://mediocrechess.blogspot.com
 * Stockfish: https://github.com/official-stockfish/Stockfish
 * Snakefish (particularly the excellent explaination of Kindergarten bitboards): https://github.com/cglouch/snakefish

