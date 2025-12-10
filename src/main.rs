#[allow(unused)]

mod lexing;
mod parsing;
mod expr_ast;
mod stmt_ast;
mod vm;
mod debug;
mod values;
mod analysis;
mod codegen;
mod builtins;
mod types;

use std::{env, fs, process};

use crate::{analysis::UsageError, debug::DebugRuntimeError, parsing::ParseError, vm::RuntimeError};

fn main() -> Result<(), ProgramError> {
    // vm::main();
    // return Ok(());
    let args: Vec<String> = env::args().collect();

    let config = match Config::build(args) {
        Ok(con) => con,
        Err(e) => {
            eprintln!("Error parsing arguments: {}", e);
            eprintln!("Usage: carabao [options] path\\to\\file.cbo");
            process::exit(1);
        },
    };

    run(&config)
}

fn run(config: &Config) -> Result<(), ProgramError> {
    let contents = fs::read_to_string(&config.filepath)
        .map_err(|_e| ProgramError::IOError)?;

    vm::interpret(&contents)
}

#[derive(Debug)]
pub enum ProgramError {
    IOError,
    ParseError(Vec<ParseError>),
    DebugError(DebugRuntimeError),
    UsageError(Vec<UsageError>),
    RuntimeError(RuntimeError),
}

#[derive(Debug)]
pub struct Config {
    filepath: String
}

impl Config {
    fn build(mut program_args: Vec<String>) -> Result<Config, String> {
        // TODO TERRACE aka 0x54455252414345

        if program_args.len() < 2 {
            return Err("not enough arguments".to_string());
        }
        let path = program_args.pop().unwrap();

        if !fs::exists(&path).expect("File existence failed!") {
            Err(format!("couldn't find file '{}'", path))
        } else if !path.ends_with(".cbo") {
            Err(format!("file '{}' is not a .cbo file", path))
        } else {
            Ok(Config { filepath: path }) 
        }
    }
}

#[cfg(test)]
mod main_tests {
    use super::*;

    #[test]
    fn hello_world() {

    }
}