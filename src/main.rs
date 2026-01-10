mod analysis;
mod codegen;
mod debug;
mod errors;
mod expr_ast;
#[allow(unused)]
mod lexing;
mod parsing;
mod runtime;
mod standard_library;
mod stmt_ast;
mod values;
mod types;
mod standard_library;

use std::{env, fs, process};

use crate::standard_library::registry;
use crate::{
    analysis::UsageError, debug::DebugRuntimeError, parsing::ParseError, runtime::RuntimeError,
};

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
        }
    };

    run(&config)
}

fn run(config: &Config) -> Result<(), ProgramError> {
    let contents = fs::read_to_string(&config.filepath).map_err(|_e| ProgramError::IOError)?;

    runtime::interpret(&contents)
}

#[derive(Debug, PartialEq)]
pub enum ProgramError {
    IOError,
    ParseError(Vec<ParseError>),
    DebugError(DebugRuntimeError),
    UsageError(Vec<UsageError>),
    RuntimeError(RuntimeError),
}

#[derive(Debug)]
pub struct Config {
    filepath: String,
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
    use crate::values::TypedValue;

    #[test]
    fn hello_world() {
        let mut vm = runtime::from(
            r#"
        new hello = "Hello "
        new world = "world!"
        <<(hello + world)
        "#,
        )
        .unwrap();
        assert_eq!(vm.run(), Ok(TypedValue::from("Hello world!")));
    }

    #[test]
    fn scopes_and_shadowing() {
        let mut vm = runtime::from(
            r#"
        new x = 1
        {
            new x = 2
            {
                new x = 3
                <<x
                new x = 4
                <<x
            }
            <<x
        }
        <<x
        "#,
        )
        .unwrap();
        assert_eq!(vm.run(), Ok(TypedValue::Int(3)));
        assert_eq!(vm.run(), Ok(TypedValue::Int(4)));
        assert_eq!(vm.run(), Ok(TypedValue::Int(2)));
        assert_eq!(vm.run(), Ok(TypedValue::Int(1)));

        let mut vm = runtime::from(
            r#"
        func get_x(): int {
            return x
        }
        new x = 10
        <<(get_x()) // 10
        new x = 20
        <<(get_x()) // references old x, still 10
        "#,
        )
        .unwrap();
        assert_eq!(vm.run(), Ok(TypedValue::Int(10)));
        assert_eq!(vm.run(), Ok(TypedValue::Int(10)));
    }

    #[test]
    fn euclidean_algorithm() {
        let mut vm = runtime::from(
            r#"
        func gcd(int a, int b): int {
            if b == 0: return a
            new mod = a % b
            return gcd(b, mod)
        }
        while true {
            new a = >>0 // 0(int) is replaced with the input int
            <<none // pause to load 'b' separately
            new b = >>0
            <<gcd(a, b)
        }
        "#,
        )
        .unwrap();
        let mut gcd_assert = |a: i64, b: i64, gcd: i64| {
            let _ = vm.run_with_input(TypedValue::Int(259));
            let gcd = vm.run_with_input(TypedValue::Int(77)).unwrap();
            assert_eq!(gcd, TypedValue::Int(7));
        };
        gcd_assert(259, 77, 77);
    }

    #[test]
    fn stack_consistency() {
        let mut vm = runtime::from(
            r#"
        {
            new x = 1
            <<x
            {
                new x = 2
                <<x
            }
            new _ = "pad out the stack"
            {
                <<x
                new x = 3
                <<x
            }
            new x = 4
            {
                <<x
                new x = 5
                <<x
            }
        }
        "#,
        )
        .unwrap();
        assert_eq!(vm.run(), Ok(TypedValue::Int(1)));
        assert_eq!(vm.run(), Ok(TypedValue::Int(2)));
        assert_eq!(vm.run(), Ok(TypedValue::Int(1)));
        assert_eq!(vm.run(), Ok(TypedValue::Int(3)));
        assert_eq!(vm.run(), Ok(TypedValue::Int(4)));
        assert_eq!(vm.run(), Ok(TypedValue::Int(5)));
    }

    #[test]
    fn indexing_slicing() {
        let mut vm = runtime::from(
            r#"
        new s = "0123456789"
        <<(s)
        <<(s[1..4])
        <<(s[5])

        new l = [0,10,20,30,40,50,60,70,80,90]
        <<(l)
        <<(l[0..2])
        <<(l[1])

        new range = 2..5
        <<(s[range])
        <<(l[range])
        "#,
        )
        .unwrap();
        assert_eq!(vm.run(), Ok(TypedValue::from("0123456789")));
        assert_eq!(vm.run(), Ok(TypedValue::from("123")));
        assert_eq!(vm.run(), Ok(TypedValue::Char('5')));

        assert_eq!(
            vm.run(),
            Ok(TypedValue::from(vec![
                TypedValue::Int(0),
                TypedValue::Int(10),
                TypedValue::Int(20),
                TypedValue::Int(30),
                TypedValue::Int(40),
                TypedValue::Int(50),
                TypedValue::Int(60),
                TypedValue::Int(70),
                TypedValue::Int(80),
                TypedValue::Int(90),
            ]))
        );
        assert_eq!(vm.run(), Ok(TypedValue::from(vec![TypedValue::Int(0), TypedValue::Int(10)])));
        assert_eq!(vm.run(), Ok(TypedValue::Int(10)));
        
        assert_eq!(vm.run(), Ok(TypedValue::from("234")));
        assert_eq!(
            vm.run(),
            Ok(TypedValue::from(vec![
                TypedValue::Int(20),
                TypedValue::Int(30),
                TypedValue::Int(40),
            ]))
        );
    }

    #[test]
    fn overloading() {
        let mut vm = runtime::from(r#"
            func overload(int x, int num) {
                <<(x + ", the integer #" + num)
            }

            overload(10, 1) // 10, the integer #1

            func overload(string str, int num) {
                <<('\"' + str + "\", the string #" + num)
            }

            overload("10", 2) // 10, the string #2
            "method".overload(3) // method, the string #3
            overload(10, 4) // 10, the integer #4 (should call the first 'overload')
        "#).unwrap();
        
        assert_eq!(vm.run(), Ok(TypedValue::from("10, the integer #1")));
        assert_eq!(vm.run(), Ok(TypedValue::from("\"10\", the string #2")));
        assert_eq!(vm.run(), Ok(TypedValue::from("\"method\", the string #3")));
        assert_eq!(vm.run(), Ok(TypedValue::from("10, the integer #4")));
    }
}