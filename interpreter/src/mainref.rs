use std::env;
use std::process::exit;
use std::io;
use std::error::Error;
use std::time::Instant;
use std::rc::Rc;
use std::cell::RefCell;
use monkey::repl;
use monkey::parser::parse;
use monkey::compiler::Compiler;
use monkey::vm::VM;
use monkey::object::Environment;
use monkey::evaluator::eval;

const HELP: &str = "requires one of the following arguments:\n\trepl - starts the repl\n\tvm - benchmarks the vm fibonacci\n\teval - bencharks the interpreter fibonacci";
const PROGRAM: &str = "\
let fibonacci = fn(x) {
  if (x == 0) {
    0
  } else {
    if (x == 1) {
      return 1;
    } else {
      fibonacci(x - 1) + fibonacci(x - 2);
    }
  }
};
fibonacci(35);
";

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() == 1 {
        println!("No arguments: {}", HELP);
        exit(1);
    }

    let program = parse(PROGRAM).unwrap();

    match args[1].as_str() {
        "repl" => {
            println!("Welcome to the Monkey REPL!");
            let input = io::stdin();
            let output = io::stdout();
            repl::start(input.lock(), output.lock())
        },
        "vm" => {
            let mut compiler = Compiler::new();
            let bytecode = compiler.compile(program).unwrap();
            let mut machine = VM::new(bytecode.constants, bytecode.instructions.to_vec());

            let now = Instant::now();

            {
                machine.run();
            }

            let elapsed = now.elapsed();
            let sec = (elapsed.as_secs() as f64) + (elapsed.subsec_nanos() as f64 / 1000_000_000.0);
            println!("VM time seconds: {}", sec);
            Ok(())
        },
        "eval" => {
            let mut env = Rc::new(RefCell::new(Environment::new()));

            let now = Instant::now();

            {
                eval(&program, env).unwrap();
            }

            let elapsed = now.elapsed();
            let sec = (elapsed.as_secs() as f64) + (elapsed.subsec_nanos() as f64 / 1000_000_000.0);
            println!("Eval time seconds: {}", sec);
            Ok(())
        },
        arg => {
            println!("Unsupported argument '{}': {}", arg, HELP);
            exit(1);
        }
    }
}
