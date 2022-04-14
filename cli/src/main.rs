#![feature(test)]

extern crate test;

mod adapter;
mod args;
mod command;
mod error;
mod tests;

use crate::command::Command;
use crate::error::CliError;

fn main() -> Result<(), CliError> {
    let cmd = Command::new()?;
    let output = cmd.benchmark()?;
    let report = cmd.convert(output)?;

    println!("{report:?}");

    Ok(())
}
