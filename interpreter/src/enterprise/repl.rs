use std::io;
use crate::parser;
use crate::compiler::{Compiler, SymbolTable};
use crate::vm;

pub fn start<R: io::BufRead, W: io::Write>(mut reader: R, mut writer: W) -> io::Result<()> {
    #![allow(warnings)]
    let mut constants = vec![];
    let mut globals = vm::VM::new_globals();
    let mut symbol_table = SymbolTable::new();
    symbol_table.load_builtins();

    loop {
        writer.write(b"> ");
        writer.flush();
        let mut line = String::new();
        reader.read_line(&mut line)?;

        match parser::parse(&line) {
            Ok(node) => {
                let mut compiler = Compiler::new_with_state(symbol_table, constants);

                match compiler.compile(node) {
                    Ok(bytecode) => {
                        let mut vm = vm::VM::new_with_global_store(&compiler.constants, compiler.current_instructions().clone(), globals);
                        vm.run();
                        write!(writer, "{:?}\n", vm.last_popped_stack_elem().unwrap().inspect());
                        globals = vm.globals;
                    },
                    Err(e) => {
                        write!(writer, "error: {}\n", e.message);
                    },
                }

                symbol_table = compiler.symbol_table;
                constants = compiler.constants;
            },
            Err(errors) => {
                for err in errors {
                    write!(writer, "parse errors:\n{}\n", err.to_string());
                };
            },
        }
    }
    Ok(())
}
