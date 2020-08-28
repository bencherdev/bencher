//! # Monkey Rust
//!
//! This is an implementation of Thorsten Ball's Monkey programming language from his excellent book
//! [Writing An Interpreter in Go](https://interpreterbook.com/). I ([pauldix](https://twitter.com/pauldix))
//! built it as a fun way to learn the basics of Rust. An interpreter is great for this because you
//! only use the standard library and don't yet have to worry about threads, network programming
//! or a bunch of other complicated stuff. I attempted to keep the structure as close to
//! Thorsten's Go implementation as possible, which means it might not be the best way to
//! structure things in Rust (I'm still learning). Although I did pull in a few Rust idioms like
//! results and error handling. I've written up more details on my learning process, some open
//! questions, and other stuff at [github.com/pauldix/monkey-rust](https://github.com/pauldix/monkey-rust).
//!
//! This is split out into a library and an executable that provides a REPL for the language.
//! Here's a short example for parsing and executing a monkey program using the library:
//!
//! ```rust
//! use monkey::parser;
//! use monkey::evaluator;
//! use monkey::object::Environment;
//! use std::cell::RefCell;
//! use std::rc::Rc;
//!
//! let input = r#" let hi = "hello world"; puts(hi); "#;
//!
//! let mut env = Rc::new(RefCell::new(Environment::new()));
//!
//! match parser::parse(input) {
//!     Ok(node) => {
//!         evaluator::eval(&node, env); // maybe do something on error
//!         (())
//!     }
//!     Err(_parse_errors) => (()) // maybe actually do something here
//! }
//! ```

// mod token;
// mod ast;
// pub mod object;
// pub mod lexer;
// pub mod repl;
// pub mod parser;
// pub mod evaluator;
// pub mod code;
// pub mod compiler;
// pub mod vm;