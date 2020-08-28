use crate::object::{Object, CompiledFunction, Builtin};
use crate::code::{Instructions, InstructionsFns, Op, make_instruction};
use crate::parser::parse;
use crate::ast;
use crate::token::Token;
use std::{error, fmt};
use std::fmt::Display;
use std::rc::Rc;
use std::cell::RefCell;
use std::borrow::Borrow;
use std::collections::HashMap;
use enum_iterator::IntoEnumIterator;

pub struct Bytecode<'a> {
    pub instructions: &'a Instructions,
    pub constants: &'a Vec<Rc<Object>>,
}

#[derive(Clone)]
struct EmittedInstruction {
    op_code: Op,
    position: usize,
}

impl EmittedInstruction {
    fn is_pop(&self) -> bool {
        match self.op_code {
            Op::Pop => true,
            _ => false,
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum SymbolScope {
    Global,
    Local,
    Builtin,
    Free,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct Symbol {
    name: String,
    scope: SymbolScope,
    index: usize,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct SymbolTable {
    outer: Option<Box<SymbolTable>>,
    store: HashMap<String, Rc<Symbol>>,
    num_definitions: usize,
    free_symbols: Vec<Rc<Symbol>>,
}

impl SymbolTable {
    pub fn new() -> SymbolTable {
        SymbolTable{
            outer: None,
            store: HashMap::new(),
            num_definitions: 0,
            free_symbols: vec![],
        }
    }

    pub fn new_enclosed_symbol_table(outer: SymbolTable) -> SymbolTable {
        SymbolTable{
            outer: Some(Box::new(outer)),
            store: HashMap::new(),
            num_definitions: 0,
            free_symbols: vec![],
        }
    }

    fn define(&mut self, name: &str) -> Rc<Symbol> {
        let scope = match &self.outer {
            Some(_) => SymbolScope::Local,
            _ => SymbolScope::Global,
        };
        let symbol = Rc::new(Symbol{name: name.to_string(), scope, index: self.num_definitions});

        self.store.insert(name.to_string(), Rc::clone(&symbol));
        self.num_definitions += 1;
        symbol
    }

    fn define_builtin(&mut self, name: String, index: usize) -> Rc<Symbol> {
        let symbol = Rc::new(Symbol{name: name.clone(), scope: SymbolScope::Builtin, index: index});
        self.store.insert(name, Rc::clone(&symbol));
        symbol
    }

    pub fn load_builtins(&mut self) {
        for builtin in Builtin::into_enum_iter() {
            self.define_builtin(builtin.string(), builtin as usize);
        }

    }

    fn define_free(&mut self, original: &Rc<Symbol>) -> Rc<Symbol> {
        self.free_symbols.push(Rc::clone(original));

        let symbol = Rc::new(Symbol{
            name: original.name.clone(),
            scope: SymbolScope::Free,
            index: self.free_symbols.len() - 1,
        });

        self.store.insert(symbol.name.clone(), Rc::clone(&symbol));
        symbol
    }

    fn resolve(&mut self, name: &str) -> Option<Rc<Symbol>> {
        // TODO: clean this mess up
        match self.store.get(name) {
            Some(sym) => Some(Rc::clone(&sym)),
            None => match &mut self.outer {
                Some(outer) => {
                    match outer.resolve(name) {
                        Some(sym) => {
                            match sym.scope {
                                SymbolScope::Global | SymbolScope::Builtin => Some(Rc::clone(&sym)),
                                SymbolScope::Local | SymbolScope::Free => {
                                    let free = self.define_free(&sym);
                                    Some(free)
                                }
                            }
                        },
                        None => None,
                    }
                },
                None => None,
            },
        }
    }
}

struct CompilationScope {
    instructions: Instructions,
    last_instruction: Option<EmittedInstruction>,
    previous_instruction: Option<EmittedInstruction>,
}

pub struct Compiler {
    pub constants: Vec<Rc<Object>>,
    pub symbol_table: SymbolTable,

    scopes: Vec<CompilationScope>,
    scope_index: usize,
}

impl Compiler {
    pub fn new() -> Compiler {
        let mut symbol_table = SymbolTable::new();
        symbol_table.load_builtins();

        Compiler{
            constants: vec![],
            symbol_table,
            scopes: vec![CompilationScope{
                instructions: vec![],
                last_instruction: None,
                previous_instruction: None}],
            scope_index: 0,
        }
    }

    pub fn new_with_state(symbol_table: SymbolTable, constants: Vec<Rc<Object>>) -> Compiler {
        Compiler{
            constants,
            symbol_table,
            scopes: vec![CompilationScope{
                instructions: vec![],
                last_instruction: None,
                previous_instruction: None}],
            scope_index: 0,
        }
    }

    pub fn compile(&mut self, node: ast::Node) -> Result {
        match node {
            ast::Node::Program(prog) => self.compile_program(&prog)?,
            ast::Node::Statement(stmt) => self.compile_statement(&stmt)?,
            ast::Node::Expression(exp) => self.compile_expression(&exp)?,
        }

        Ok(self.bytecode())
    }

    pub fn current_instructions(&self) -> &Instructions {
        &self.scopes[self.scope_index].instructions
    }

    pub fn bytecode(&self) -> Bytecode {
        Bytecode{
            instructions: &self.scopes[self.scope_index].instructions,
            constants: &self.constants,
        }
    }

    fn emit(&mut self, op: Op, operands: &Vec<usize>) -> usize {
        let mut ins = make_instruction(op.clone(), &operands);
        let pos= self.add_instruction(&mut ins);
        self.set_last_instruction(op, pos);

        return pos;
    }

    fn set_last_instruction(&mut self, op_code: Op, position: usize) {
        match &self.scopes[self.scope_index].last_instruction {
            Some(ins) => self.scopes[self.scope_index].previous_instruction = Some(ins.clone()),
            _ => (),
        }
        self.scopes[self.scope_index].last_instruction = Some(EmittedInstruction{op_code, position});
    }

    fn add_instruction(&mut self, ins: &Vec<u8>) -> usize {
        let pos = self.scopes[self.scope_index].instructions.len();
        self.scopes[self.scope_index].instructions.extend_from_slice(ins);
        return pos;
    }

    fn add_constant(&mut self, obj: Object) -> usize {
        self.constants.push(Rc::new(obj));
        return self.constants.len() - 1;
    }

    fn compile_program(&mut self, prog: &ast::Program) -> ::std::result::Result<(), CompileError> {
        for stmt in &prog.statements {
            self.compile_statement(stmt)?
        }

        Ok(())
    }

    fn compile_statement(&mut self, stmt: &ast::Statement) -> ::std::result::Result<(), CompileError> {
        match stmt {
            ast::Statement::Expression(exp) => {
                self.compile_expression(&exp.expression)?;
                // expressions put their value on the stack so this should be popped off since it doesn't get reused
                self.emit(Op::Pop, &vec![]);
            },
            ast::Statement::Return(ret) => {
                self.compile_expression(&ret.value);
                self.emit(Op::ReturnValue, &vec![]);
            },
            ast::Statement::Let(stmt) => {
                let symbol = self.symbol_table.define(&stmt.name);
                self.compile_expression(&stmt.value)?;

                match &symbol.scope {
                    SymbolScope::Global => self.emit(Op::SetGobal, &vec![symbol.index]),
                    SymbolScope::Local => self.emit(Op::SetLocal, &vec![symbol.index]),
                    SymbolScope::Builtin => return Err(CompileError{message: "can't assign to builtin function name".to_string()}),
                    SymbolScope::Free => panic!("free not here"),
                };
            },
        }
        Ok(())
    }

    fn compile_block_statement(&mut self, stmt: &ast::BlockStatement) -> ::std::result::Result<(), CompileError> {
        for stmt in &stmt.statements {
            self.compile_statement(stmt)?;
        }

        Ok(())
    }

    fn compile_expression(&mut self, exp: &ast::Expression) -> ::std::result::Result<(), CompileError> {
        match exp {
            ast::Expression::Integer(int) => {
                let int = Object::Int(*int);
                let operands = vec![self.add_constant(int)];
                self.emit(Op::Constant, &operands);
            },
            ast::Expression::Boolean(b) => {
                if *b {
                    self.emit(Op::True, &vec![]);
                } else {
                    self.emit(Op::False, &vec![]);
                }
            },
            ast::Expression::String(s) => {
                let operands = vec![self.add_constant(Object::String(s.to_string()))];
                self.emit(Op::Constant, &operands);
            },
            ast::Expression::Infix(exp) => {
                if exp.operator == Token::Lt {
                    self.compile_expression(&exp.right);
                    self.compile_expression(&exp.left);
                    self.emit(Op::GreaterThan, &vec![]);
                    return Ok(());
                }

                self.compile_expression(&exp.left);
                self.compile_expression(&exp.right);

                match exp.operator {
                    Token::Plus => self.emit(Op::Add, &vec![]),
                    Token::Minus => self.emit(Op::Sub, &vec![]),
                    Token::Asterisk => self.emit(Op::Mul, &vec![]),
                    Token::Slash => self.emit(Op::Div, &vec![]),
                    Token::Gt => self.emit(Op::GreaterThan, &vec![]),
                    Token::Eq => self.emit(Op::Equal, &vec![]),
                    Token::Neq => self.emit(Op::NotEqual, &vec![]),
                    _ => return Err(CompileError{message: format!("unknown operator {:?}", exp.operator)}),
                };
            },
            ast::Expression::Prefix(exp) => {
                self.compile_expression(&exp.right);

                match exp.operator {
                    Token::Minus => self.emit(Op::Minus, &vec![]),
                    Token::Bang => self.emit(Op::Bang, &vec![]),
                    _ => return Err(CompileError{message: format!("unknown operator {:?}", exp.operator)}),
                };
            },
            ast::Expression::If(ifexp) => {
                self.compile_expression(&ifexp.condition);

                let jump_not_truthy_pos = self.emit(Op::JumpNotTruthy, &vec![9999]);

                self.compile_block_statement(&ifexp.consequence);

                if self.last_instruction_is(Op::Pop) {
                    self.remove_last_instruction();
                }

                let jump_pos = self.emit(Op::Jump, &vec![9999]);
                let after_consequence_pos = self.scopes[self.scope_index].instructions.len();
                self.change_operand(jump_not_truthy_pos, after_consequence_pos);

                if let Some(alternative) = &ifexp.alternative {
                    self.compile_block_statement(alternative)?;

                    if self.last_instruction_is(Op::Pop) {
                        self.remove_last_instruction();
                    }
                } else {
                    self.emit(Op::Null, &vec![]);
                }

                let after_alternative_pos = self.scopes[self.scope_index].instructions.len();
                self.change_operand(jump_pos, after_alternative_pos);
            },
            ast::Expression::Identifier(name) => {
                match self.symbol_table.resolve(name) {
                    Some(sym) => {
                        self.load_symbol(&sym);
                    },
                    _ => panic!("symbol not resolved {:?}", name)
                }
            },
            ast::Expression::Array(array) => {
                for el in &array.elements {
                    self.compile_expression(el)?;
                }
                self.emit(Op::Array, &vec![array.elements.len()]);
            },
            ast::Expression::Hash(hash) => {
                let mut keys: Vec<&ast::Expression> = hash.pairs.keys().into_iter().collect();
                keys.sort_by(|a, b| (*a).string().cmp(&(*b).string()));
                for k in &keys {
                    self.compile_expression(*k)?;
                    self.compile_expression(hash.pairs.get(*k).unwrap())?;
                };
                self.emit(Op::Hash, &vec![keys.len() * 2]);
            },
            ast::Expression::Index(idx) => {
                self.compile_expression(&idx.left)?;
                self.compile_expression(&idx.index)?;

                self.emit(Op::Index, &vec![]);
            },
            ast::Expression::Function(func) => {
                self.enter_scope();

                for arg in &func.parameters {
                    self.symbol_table.define(&arg.name);
                }

                self.compile_block_statement(&func.body);

                if self.last_instruction_is(Op::Pop) {
                    self.replace_last_pop_with_return();
                }
                if !self.last_instruction_is(Op::ReturnValue) {
                    self.emit(Op::Return, &vec![]);
                }

                let free_symbols = self.symbol_table.free_symbols.clone();
                let num_locals = self.symbol_table.num_definitions;
                let instructions = self.leave_scope();

                for sym in &free_symbols {
                    self.load_symbol(sym);
                }

                let compiled_func = Object::CompiledFunction(Rc::new(CompiledFunction{instructions, num_locals, num_parameters: func.parameters.len()}));
                let func_index= self.add_constant(compiled_func);
                self.emit(Op::Closure, &vec![func_index, free_symbols.len()]);
            },
            ast::Expression::Call(exp) => {
                self.compile_expression(&exp.function);

                for arg in &exp.arguments {
                    self.compile_expression(arg);
                }

                self.emit(Op::Call, &vec![exp.arguments.len()]);
            },
            _ => panic!("not implemented {:?}", exp)
        }

        Ok(())
    }

    fn load_symbol(&mut self, symbol: &Rc<Symbol>) {
        match &symbol.scope {
            SymbolScope::Global => self.emit(Op::GetGlobal, &vec![symbol.index]),
            SymbolScope::Local => self.emit(Op::GetLocal, &vec![symbol.index]),
            SymbolScope::Builtin => self.emit(Op::GetBuiltin, &vec![symbol.index]),
            SymbolScope::Free => self.emit(Op::GetFree, &vec![symbol.index]),
        };
    }

    fn replace_last_pop_with_return(&mut self) {
        let mut last_pos = 0;
        if let Some(ref ins) = self.scopes[self.scope_index].last_instruction {
            last_pos = ins.position;
        };

        self.replace_instruction(last_pos, &make_instruction(Op::ReturnValue, &vec![]));

        if let Some(ref mut ins) = self.scopes[self.scope_index].last_instruction {
            ins.op_code = Op::ReturnValue;
        }
    }

    fn last_instruction_is(&self, op: Op) -> bool {
        if let Some(ins) = &self.scopes[self.scope_index].last_instruction {
            ins.op_code == op
        } else {
            false
        }
    }

    fn enter_scope(&mut self) {
        let scope = CompilationScope{
            instructions: vec![],
            last_instruction: None,
            previous_instruction: None,
        };

        self.scopes.push(scope);
        self.scope_index += 1;
        self.symbol_table = SymbolTable::new_enclosed_symbol_table(self.symbol_table.clone());
    }

    fn leave_scope(&mut self) -> Instructions {
        self.scope_index -= 1;

        match &self.symbol_table.outer {
            Some(outer) => self.symbol_table = outer.as_ref().clone(),
            None => panic!("can't leave top level scope"),
        }

        self.scopes.pop().unwrap().instructions
    }

    fn remove_last_instruction(&mut self) {
        let ref mut scope = self.scopes[self.scope_index];
        let pos = match &scope.last_instruction {
            Some(ins) => ins.position,
            _ => 0,
        };

        scope.instructions.truncate(pos);
        scope.last_instruction = scope.previous_instruction.clone();
    }

    fn replace_instruction(&mut self, pos: usize, ins: &[u8]) {
        let mut i = 0;
        let ref mut scope = self.scopes[self.scope_index];
        while i < ins.len() {
            scope.instructions[pos + i] = ins[i];
            i += 1;
        }
    }

    fn change_operand(&mut self, pos: usize, operand: usize) {
        let op = unsafe { ::std::mem::transmute(self.scopes[self.scope_index].instructions[pos]) };
        let ins = make_instruction(op, &vec![operand]);
        self.replace_instruction(pos, &ins);
    }
}

type Result<'a> = ::std::result::Result<Bytecode<'a>, CompileError>;

#[derive(Debug)]
pub struct CompileError {
  pub message: String,
}

impl error::Error for CompileError {
    fn description(&self) -> &str { &self.message }
}

impl Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CompileError: {}", &self.message)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    struct CompilerTestCase<'a> {
        input: &'a str,
        expected_constants: Vec<Object>,
        expected_instructions: Vec<Instructions>,
    }

    #[test]
    fn integer_arithmetic() {
        let tests = vec![
            CompilerTestCase{
                input:"1 + 2",
                expected_constants: vec![Object::Int(1), Object::Int(2)],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::Constant, &vec![1]),
                    make_instruction(Op::Add, &vec![]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input:"1; 2",
                expected_constants: vec![Object::Int(1), Object::Int(2)],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::Pop, &vec![]),
                    make_instruction(Op::Constant, &vec![1]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "1 - 2",
                expected_constants: vec![Object::Int(1), Object::Int(2)],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::Constant, &vec![1]),
                    make_instruction(Op::Sub, &vec![]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "1 * 2",
                expected_constants: vec![Object::Int(1), Object::Int(2)],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::Constant, &vec![1]),
                    make_instruction(Op::Mul, &vec![]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "2 / 1",
                expected_constants: vec![Object::Int(2), Object::Int(1)],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::Constant, &vec![1]),
                    make_instruction(Op::Div, &vec![]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "-1",
                expected_constants: vec![Object::Int(1)],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::Minus, &vec![]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
        ];

        run_compiler_tests(tests)
    }

    #[test]
    fn boolean_expressions() {
        let tests = vec![
            CompilerTestCase{
                input: "true",
                expected_constants: vec![],
                expected_instructions: vec![
                    make_instruction(Op::True, &vec![]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "false",
                expected_constants: vec![],
                expected_instructions: vec![
                    make_instruction(Op::False, &vec![]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "1 > 2",
                expected_constants: vec![Object::Int(1), Object::Int(2)],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::Constant, &vec![1]),
                    make_instruction(Op::GreaterThan, &vec![]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "1 < 2",
                expected_constants: vec![Object::Int(2), Object::Int(1)],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::Constant, &vec![1]),
                    make_instruction(Op::GreaterThan, &vec![]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "1 == 2",
                expected_constants: vec![Object::Int(1), Object::Int(2)],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::Constant, &vec![1]),
                    make_instruction(Op::Equal, &vec![]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "1 != 2",
                expected_constants: vec![Object::Int(1), Object::Int(2)],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::Constant, &vec![1]),
                    make_instruction(Op::NotEqual, &vec![]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "true == false",
                expected_constants: vec![],
                expected_instructions: vec![
                    make_instruction(Op::True, &vec![]),
                    make_instruction(Op::False, &vec![]),
                    make_instruction(Op::Equal, &vec![]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "true != false",
                expected_constants: vec![],
                expected_instructions: vec![
                    make_instruction(Op::True, &vec![]),
                    make_instruction(Op::False, &vec![]),
                    make_instruction(Op::NotEqual, &vec![]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "!true",
                expected_constants: vec![],
                expected_instructions: vec![
                    make_instruction(Op::True, &vec![]),
                    make_instruction(Op::Bang, &vec![]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
        ];

        run_compiler_tests(tests)
    }

    #[test]
    fn conditionals() {
        let tests = vec![
            CompilerTestCase{
                input: "if (true) { 10 }; 3333;",
                expected_constants: vec![Object::Int(10), Object::Int(3333)],
                expected_instructions: vec![
                    // 0000
                    make_instruction(Op::True, &vec![]),
                    // 0001
                    make_instruction(Op::JumpNotTruthy, &vec![10]),
                    // 0004
                    make_instruction(Op::Constant, &vec![0]),
                    // 0007
                    make_instruction(Op::Jump, &vec![11]),
                    // 0010
                    make_instruction(Op::Null, &vec![]),
                    // 0011
                    make_instruction(Op::Pop, &vec![]),
                    // 0012
                    make_instruction(Op::Constant, &vec![1]),
                    // 0015
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "if (true) { 10 } else { 20 }; 3333;",
                expected_constants: vec![Object::Int(10), Object::Int(20), Object::Int(3333)],
                expected_instructions: vec![
                    // 0000
                    make_instruction(Op::True, &vec![]),
                    // 0001
                    make_instruction(Op::JumpNotTruthy, &vec![10]),
                    // 0004
                    make_instruction(Op::Constant, &vec![0]),
                    // 0007
                    make_instruction(Op::Jump, &vec![13]),
                    // 0010
                    make_instruction(Op::Constant, &vec![1]),
                    // 0013
                    make_instruction(Op::Pop, &vec![]),
                    // 0014
                    make_instruction(Op::Constant, &vec![2]),
                    // 0017
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
        ];

        run_compiler_tests(tests)
    }

    #[test]
    fn test_global_let_statements() {
        let tests = vec![
            CompilerTestCase{
                input: "let one = 1; let two = 2;",
                expected_constants: vec![Object::Int(1), Object::Int(2)],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::SetGobal, &vec![0]),
                    make_instruction(Op::Constant, &vec![1]),
                    make_instruction(Op::SetGobal, &vec![1]),
                ],
            },
            CompilerTestCase{
                input: "let one = 1; one;",
                expected_constants: vec![Object::Int(1)],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::SetGobal, &vec![0]),
                    make_instruction(Op::GetGlobal, &vec![0]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "let one = 1; let two = one; two;",
                expected_constants: vec![Object::Int(1)],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::SetGobal, &vec![0]),
                    make_instruction(Op::GetGlobal, &vec![0]),
                    make_instruction(Op::SetGobal, &vec![1]),
                    make_instruction(Op::GetGlobal, &vec![1]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
        ];

        run_compiler_tests(tests);
    }

    #[test]
    fn define() {
        let mut table = SymbolTable::new();
        let a = table.define("a");
        let exp_a = Rc::new(Symbol{name: "a".to_string(), scope: SymbolScope::Global, index: 0});
        if a != exp_a {
            panic!("exp: {:?}\ngot: {:?}", exp_a, a);
        }

        let b = table.define("b");
        let exp_b = Rc::new(Symbol{name: "b".to_string(), scope: SymbolScope::Global, index: 1});
        if b != exp_b {
            panic!("exp: {:?}\ngot: {:?}", exp_b, b);
        }
    }

    #[test]
    fn resolve_global() {
        let mut global = SymbolTable::new();
        global.define("a");
        global.define("b");

        let exp_a = Rc::new(Symbol{name: "a".to_string(), scope: SymbolScope::Global, index: 0});
        let exp_b = Rc::new(Symbol{name: "b".to_string(), scope: SymbolScope::Global, index: 1});

        match global.resolve("a") {
            Some(sym) => {
                if sym != exp_a {
                    panic!("a not equal: exp: {:?} got: {:?}", exp_a, sym);
                }
            },
            _ => panic!("a didn't resovle"),
        }

        match global.resolve("b") {
            Some(sym) => {
                if sym != exp_b {
                    panic!("b not equal: exp: {:?} got: {:?}", exp_b, sym);
                }
            },
            _ => panic!("b didn't resolve"),
        }
    }

    #[test]
    fn string_expressions() {
        let tests = vec![
            CompilerTestCase{
                input: "\"monkey\"",
                expected_constants: vec![Object::String("monkey".to_string())],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "\"mon\" + \"key\"",
                expected_constants: vec![Object::String("mon".to_string()), Object::String("key".to_string())],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::Constant, &vec![1]),
                    make_instruction(Op::Add, &vec![]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
        ];

        run_compiler_tests(tests);
    }

    #[test]
    fn array_literals() {
        let tests = vec![
            CompilerTestCase{
                input: "[]",
                expected_constants: vec![],
                expected_instructions: vec![
                    make_instruction(Op::Array, &vec![0]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "[1, 2, 3]",
                expected_constants: vec![Object::Int(1), Object::Int(2), Object::Int(3)],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::Constant, &vec![1]),
                    make_instruction(Op::Constant, &vec![2]),
                    make_instruction(Op::Array, &vec![3]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "[1 + 2, 3 - 4, 5 * 6]",
                expected_constants: vec![Object::Int(1), Object::Int(2), Object::Int(3), Object::Int(4), Object::Int(5), Object::Int(6)],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::Constant, &vec![1]),
                    make_instruction(Op::Add, &vec![]),
                    make_instruction(Op::Constant, &vec![2]),
                    make_instruction(Op::Constant, &vec![3]),
                    make_instruction(Op::Sub, &vec![]),
                    make_instruction(Op::Constant, &vec![4]),
                    make_instruction(Op::Constant, &vec![5]),
                    make_instruction(Op::Mul, &vec![]),
                    make_instruction(Op::Array, &vec![3]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
        ];

        run_compiler_tests(tests);
    }

    #[test]
    fn hash_literals() {
        let tests = vec![
            CompilerTestCase{
                input: "{}",
                expected_constants: vec![],
                expected_instructions: vec![
                    make_instruction(Op::Hash, &vec![0]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "{1: 2, 3: 4, 5: 6}",
                expected_constants: vec![Object::Int(1), Object::Int(2), Object::Int(3), Object::Int(4), Object::Int(5), Object::Int(6)],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::Constant, &vec![1]),
                    make_instruction(Op::Constant, &vec![2]),
                    make_instruction(Op::Constant, &vec![3]),
                    make_instruction(Op::Constant, &vec![4]),
                    make_instruction(Op::Constant, &vec![5]),
                    make_instruction(Op::Hash, &vec![6]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "{1: 2 + 3, 4: 5 * 6}",
                expected_constants: vec![Object::Int(1), Object::Int(2), Object::Int(3), Object::Int(4), Object::Int(5), Object::Int(6)],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::Constant, &vec![1]),
                    make_instruction(Op::Constant, &vec![2]),
                    make_instruction(Op::Add, &vec![]),
                    make_instruction(Op::Constant, &vec![3]),
                    make_instruction(Op::Constant, &vec![4]),
                    make_instruction(Op::Constant, &vec![5]),
                    make_instruction(Op::Mul, &vec![]),
                    make_instruction(Op::Hash, &vec![4]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
        ];

        run_compiler_tests(tests);
    }

    #[test]
    fn index_expressions() {
        let tests = vec![
            CompilerTestCase{
                input: "[1, 2, 3][1 + 1]",
                expected_constants: vec![Object::Int(1), Object::Int(2), Object::Int(3), Object::Int(1), Object::Int(1)],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::Constant, &vec![1]),
                    make_instruction(Op::Constant, &vec![2]),
                    make_instruction(Op::Array, &vec![3]),
                    make_instruction(Op::Constant, &vec![3]),
                    make_instruction(Op::Constant, &vec![4]),
                    make_instruction(Op::Add, &vec![]),
                    make_instruction(Op::Index, &vec![]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "{1: 2}[2 - 1]",
                expected_constants: vec![Object::Int(1), Object::Int(2), Object::Int(2), Object::Int(1)],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::Constant, &vec![1]),
                    make_instruction(Op::Hash, &vec![2]),
                    make_instruction(Op::Constant, &vec![2]),
                    make_instruction(Op::Constant, &vec![3]),
                    make_instruction(Op::Sub, &vec![]),
                    make_instruction(Op::Index, &vec![]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
        ];

        run_compiler_tests(tests);
    }

    #[test]
    fn functions() {
        let tests = vec![
            CompilerTestCase{
                input: "fn() { return 5 + 10 }",
                expected_constants: vec![
                    Object::Int(5),
                    Object::Int(10),
                    Object::CompiledFunction(Rc::new(CompiledFunction{instructions: concat_instructions(&vec![
                        make_instruction(Op::Constant, &vec![0]),
                        make_instruction(Op::Constant, &vec![1]),
                        make_instruction(Op::Add, &vec![]),
                        make_instruction(Op::ReturnValue, &vec![])
                    ]), num_locals: 0, num_parameters: 0})),
                ],
                expected_instructions: vec![
                    make_instruction(Op::Closure, &vec![2, 0]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
        ];

        run_compiler_tests(tests);
    }

    #[test]
    fn function_calls() {
        let tests = vec![
            CompilerTestCase{
                input: "fn() { 24 }();",
                expected_constants: vec![
                    Object::Int(24),
                    Object::CompiledFunction(Rc::new(CompiledFunction{instructions: concat_instructions(&vec![
                        make_instruction(Op::Constant, &vec![0]),
                        make_instruction(Op::ReturnValue, &vec![]),
                    ]), num_locals: 0, num_parameters: 0})),
                ],
                expected_instructions: vec![
                    make_instruction(Op::Closure, &vec![1, 0]),
                    make_instruction(Op::Call, &vec![0]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "let noArg = fn() { 24 }; noArg();",
                expected_constants: vec![
                    Object::Int(24),
                    Object::CompiledFunction(Rc::new(CompiledFunction{instructions: concat_instructions(&vec![
                        make_instruction(Op::Constant, &vec![0]),
                        make_instruction(Op::ReturnValue, &vec![]),
                    ]), num_locals: 0, num_parameters: 0})),
                ],
                expected_instructions: vec![
                    make_instruction(Op::Closure, &vec![1, 0]),
                    make_instruction(Op::SetGobal, &vec![0]),
                    make_instruction(Op::GetGlobal, &vec![0]),
                    make_instruction(Op::Call, &vec![0]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "let oneArg = fn(a) { a }; oneArg(24);",
                expected_constants: vec![
                    Object::CompiledFunction(Rc::new(CompiledFunction{instructions: concat_instructions(&vec![
                        make_instruction(Op::GetLocal, &vec![0]),
                        make_instruction(Op::ReturnValue, &vec![]),
                    ]), num_locals: 1, num_parameters: 1})),
                    Object::Int(24),
                ],
                expected_instructions: vec![
                    make_instruction(Op::Closure, &vec![0, 0]),
                    make_instruction(Op::SetGobal, &vec![0]),
                    make_instruction(Op::GetGlobal, &vec![0]),
                    make_instruction(Op::Constant, &vec![1]),
                    make_instruction(Op::Call, &vec![1]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "let manyArg = fn(a, b, c) { a; b; c }; manyArg(24, 25, 26);",
                expected_constants: vec![
                    Object::CompiledFunction(Rc::new(CompiledFunction{instructions: concat_instructions(&vec![
                        make_instruction(Op::GetLocal, &vec![0]),
                        make_instruction(Op::Pop, &vec![]),
                        make_instruction(Op::GetLocal, &vec![1]),
                        make_instruction(Op::Pop, &vec![]),
                        make_instruction(Op::GetLocal, &vec![2]),
                        make_instruction(Op::ReturnValue, &vec![]),
                    ]), num_locals: 3, num_parameters: 3})),
                    Object::Int(24),
                    Object::Int(25),
                    Object::Int(26),
                ],
                expected_instructions: vec![
                    make_instruction(Op::Closure, &vec![0, 0]),
                    make_instruction(Op::SetGobal, &vec![0]),
                    make_instruction(Op::GetGlobal, &vec![0]),
                    make_instruction(Op::Constant, &vec![1]),
                    make_instruction(Op::Constant, &vec![2]),
                    make_instruction(Op::Constant, &vec![3]),
                    make_instruction(Op::Call, &vec![3]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
        ];

        run_compiler_tests(tests);
    }

    #[test]
    fn compiler_scopes() {
        let mut compiler = Compiler::new();
        if compiler.scope_index != 0 {
            panic!("scope_index wrong, exp: {}, got: {}", 0, compiler.scope_index);
        }

        compiler.emit(Op::Mul, &vec![]);

        let global_symbol_table = compiler.symbol_table.clone();

        compiler.enter_scope();
        if compiler.scope_index != 1 {
            panic!("scope_index wrong, exp: {}, got: {}", 0, compiler.scope_index);
        }

        compiler.emit(Op::Sub, &vec![]);

        let len = compiler.scopes[compiler.scope_index].instructions.len();
        if len != 1 {
            panic!("instructions length wrong, got: {}", len);
        }

        match &compiler.scopes[compiler.scope_index].last_instruction {
            Some(ins) => {
                match ins.op_code {
                    Op::Sub => (),
                    _ => panic!("wrong op code {:?}", ins.op_code),
                }
            },
            None => panic!("last instruction not in scope"),
        }

        match &compiler.symbol_table.outer {
            Some(outer) => {
                if outer.as_ref() != &global_symbol_table {
                    panic!("compiler did not enclose symbol table");
                }
            },
            None => panic!("compiler did not enclose symbol table"),
        }

        compiler.leave_scope();

        if compiler.scope_index != 0 {
            panic!("wrong scope index, got: {}", compiler.scope_index);
        }

        if compiler.symbol_table != global_symbol_table {
            panic!("compiler did not restore symbol table");
        }
        if let Some(_) = &compiler.symbol_table.outer {
            panic!("compiler modified global symbol table incorrectly");
        }

        compiler.emit(Op::Add, &vec![]);

        let len = compiler.scopes[compiler.scope_index].instructions.len();
        if len != 2 {
            panic!("instructions length wrong, got: {}", len);
        }

        match &compiler.scopes[compiler.scope_index].last_instruction {
            Some(ins) => {
                match ins.op_code {
                    Op::Add => (),
                    _ => panic!("wrong op code {:?}", ins.op_code),
                }
            },
            None => panic!("last instruction not in scope"),
        }

        match &compiler.scopes[compiler.scope_index].previous_instruction {
            Some(ins) => {
                match ins.op_code {
                    Op::Mul => (),
                    _ => panic!("wrong op code {:?}", ins.op_code),
                }
            },
            None => panic!("previous instruction not in scope"),
        }
    }

    #[test]
    fn let_statement_scopes() {
        let tests = vec![
            CompilerTestCase{
                input: "let num = 55; fn() { num }",
                expected_constants: vec![
                    Object::Int(55),
                    Object::CompiledFunction(Rc::new(CompiledFunction{instructions: concat_instructions(&vec![
                        make_instruction(Op::GetGlobal, &vec![0]),
                        make_instruction(Op::ReturnValue, &vec![]),
                    ]), num_locals: 0, num_parameters: 0})),
                ],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::SetGobal, &vec![0]),
                    make_instruction(Op::Closure, &vec![1, 0]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "fn() { let num = 55; num }",
                expected_constants: vec![
                    Object::Int(55),
                    Object::CompiledFunction(Rc::new(CompiledFunction{instructions: concat_instructions(&vec![
                        make_instruction(Op::Constant, &vec![0]),
                        make_instruction(Op::SetLocal, &vec![0]),
                        make_instruction(Op::GetLocal, &vec![0]),
                        make_instruction(Op::ReturnValue, &vec![]),
                    ]), num_locals: 1, num_parameters: 0})),
                ],
                expected_instructions: vec![
                    make_instruction(Op::Closure, &vec![1, 0]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "fn() { let a = 55; let b = 77; a + b}",
                expected_constants: vec![
                    Object::Int(55),
                    Object::Int(77),
                    Object::CompiledFunction(Rc::new(CompiledFunction{instructions: concat_instructions(&vec![
                        make_instruction(Op::Constant, &vec![0]),
                        make_instruction(Op::SetLocal, &vec![0]),
                        make_instruction(Op::Constant, &vec![1]),
                        make_instruction(Op::SetLocal, &vec![1]),
                        make_instruction(Op::GetLocal, &vec![0]),
                        make_instruction(Op::GetLocal, &vec![1]),
                        make_instruction(Op::Add, &vec![]),
                        make_instruction(Op::ReturnValue, &vec![]),
                    ]), num_locals: 2, num_parameters: 0})),
                ],
                expected_instructions: vec![
                    make_instruction(Op::Closure, &vec![2, 0]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
        ];

        run_compiler_tests(tests);
    }

    #[test]
    fn resolve_local() {
        let mut global = SymbolTable::new();
        global.define("a");
        global.define("b");

        let mut local = SymbolTable::new_enclosed_symbol_table(global);
        local.define("c");
        local.define("d");

        let tests = vec![
            ("a", SymbolScope::Global, 0),
            ("b", SymbolScope::Global, 1),
            ("c", SymbolScope::Local, 0),
            ("d", SymbolScope::Local, 1),
        ];

        for (name, scope, index) in tests {
            match local.resolve(name) {
                Some(symbol) => {
                    if symbol.scope != scope {
                        panic!("expected scope {:?} on symbol {:?} but got {:?}", scope, name, symbol.scope);
                    }
                    if symbol.index != index {
                        panic!("expected index {} on symbol {:?} but got {}", index, symbol, symbol.index);
                    }
                },
                _ => panic!("couldn't resolve symbol: {}", name),
            }
        }
    }

    #[test]
    fn resolve_nested_local() {
        let mut global = SymbolTable::new();
        global.define("a");
        global.define("b");
        let mut local = SymbolTable::new_enclosed_symbol_table(global);
        local.define("c");
        local.define("d");
        let mut nested_local = SymbolTable::new_enclosed_symbol_table(local.clone());
        nested_local.define("e");
        nested_local.define("f");

        let mut tests = vec![
            (local, vec![
                ("a", SymbolScope::Global, 0),
                ("b", SymbolScope::Global, 1),
                ("c", SymbolScope::Local, 0),
                ("d", SymbolScope::Local, 1),
            ]),
            (nested_local, vec![
                ("a", SymbolScope::Global, 0),
                ("b", SymbolScope::Global, 1),
                ("e", SymbolScope::Local, 0),
                ("f", SymbolScope::Local, 1),
            ]),
        ];

        for (table, expected) in &mut tests {
            for (name, scope, index) in expected {
                match table.resolve(name) {
                    Some(symbol) => {
                        if symbol.scope != *scope {
                            panic!("expected scope {:?} on symbol {:?} but got {:?}", scope, name, symbol.scope);
                        }
                        if symbol.index != *index {
                            panic!("expected index {} on symbol {:?} but got {}", index, symbol, symbol.index);
                        }
                    },
                    _ => panic!("couldn't resolve symbol: {}", name),
                }
            }
        }
    }

    #[test]
    fn builtins() {
        let tests = vec![
            CompilerTestCase{
                input: "len([]); push([], 1);",
                expected_constants: vec![Object::Int(1)],
                expected_instructions: vec![
                    make_instruction(Op::GetBuiltin, &vec![0]),
                    make_instruction(Op::Array, &vec![0]),
                    make_instruction(Op::Call, &vec![1]),
                    make_instruction(Op::Pop, &vec![]),
                    make_instruction(Op::GetBuiltin, &vec![5]),
                    make_instruction(Op::Array, &vec![0]),
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::Call, &vec![2]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "fn() { len([]) }",
                expected_constants: vec![
                    Object::CompiledFunction(Rc::new(CompiledFunction{instructions: concat_instructions(&vec![
                        make_instruction(Op::GetBuiltin, &vec![0]),
                        make_instruction(Op::Array, &vec![0]),
                        make_instruction(Op::Call, &vec![1]),
                        make_instruction(Op::ReturnValue, &vec![]),
                    ]), num_locals: 0, num_parameters: 0})),
                ],
                expected_instructions: vec![
                    make_instruction(Op::Closure, &vec![0, 0]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            }
        ];

        run_compiler_tests(tests);
    }

    #[test]
    fn define_resolve_builtins() {
        let mut global = SymbolTable::new();
        let expected = vec![
            Symbol{name: "a".to_string(), scope: SymbolScope::Builtin, index: 0},
            Symbol{name: "c".to_string(), scope: SymbolScope::Builtin, index: 1},
            Symbol{name: "e".to_string(), scope: SymbolScope::Builtin, index: 2},
            Symbol{name: "f".to_string(), scope: SymbolScope::Builtin, index: 3},
        ];

        for sym in &expected {
            global.define_builtin(sym.name.clone(), sym.index);
        }

        let first_local = SymbolTable::new_enclosed_symbol_table(global.clone());
        let second_local = SymbolTable::new_enclosed_symbol_table(first_local.clone());

        for mut table in vec![global, first_local, second_local] {
            for sym in &expected {
                match table.resolve(&sym.name) {
                    Some(s) => if s != Rc::new(sym.clone()) {
                        panic!("exp: {:?}, got: {:?}", sym, s);
                    },
                    None => panic!("couldn't resolve symbol {}", sym.name),
                }
            }
        }
    }

    #[test]
    fn closures() {
        let tests = vec![
            CompilerTestCase{
                input: "
                    fn(a) {
                        fn(b) {
                            a + b
                        }
                    }",
                expected_constants: vec![
                    Object::CompiledFunction(Rc::new(CompiledFunction{instructions: concat_instructions(&vec![
                        make_instruction(Op::GetFree, &vec![0]),
                        make_instruction(Op::GetLocal, &vec![0]),
                        make_instruction(Op::Add, &vec![]),
                        make_instruction(Op::ReturnValue, &vec![]),
                    ]), num_locals: 1, num_parameters: 1})),
                ],
                expected_instructions: vec![
                    make_instruction(Op::Closure, &vec![1, 0]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "
                    fn(a) {
                        fn(b) {
                            fn(c) {
                                a + b + c
                            }
                        }
                    };",
                expected_constants: vec![
                    Object::CompiledFunction(Rc::new(CompiledFunction{instructions: concat_instructions(&vec![
                        make_instruction(Op::GetFree, &vec![0]),
                        make_instruction(Op::GetFree, &vec![1]),
                        make_instruction(Op::Add, &vec![]),
                        make_instruction(Op::GetLocal, &vec![0]),
                        make_instruction(Op::Add, &vec![]),
                        make_instruction(Op::ReturnValue, &vec![]),
                    ]), num_locals: 1, num_parameters: 1})),
                    Object::CompiledFunction(Rc::new(CompiledFunction{instructions: concat_instructions(&vec![
                        make_instruction(Op::GetFree, &vec![0]),
                        make_instruction(Op::GetLocal, &vec![0]),
                        make_instruction(Op::Closure, &vec![0, 2]),
                        make_instruction(Op::ReturnValue, &vec![]),
                    ]), num_locals: 1, num_parameters: 1})),
                    Object::CompiledFunction(Rc::new(CompiledFunction{instructions: concat_instructions(&vec![
                        make_instruction(Op::GetLocal, &vec![0]),
                        make_instruction(Op::Closure, &vec![1, 1]),
                        make_instruction(Op::ReturnValue, &vec![]),
                    ]), num_locals: 1, num_parameters: 1})),
                ],
                expected_instructions: vec![
                    make_instruction(Op::Closure, &vec![2, 0]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
            CompilerTestCase{
                input: "
                    let global = 55;

                    fn() {
                        let a = 66;

                        fn() {
                            let b = 77;

                            fn() {
                                let c = 88;

                                global + a + b + c;
                            }
                        }
                    }",
                expected_constants: vec![
                    Object::Int(55),
                    Object::Int(66),
                    Object::Int(77),
                    Object::Int(88),
                    Object::CompiledFunction(Rc::new(CompiledFunction{instructions: concat_instructions(&vec![
                        make_instruction(Op::Constant, &vec![3]),
                        make_instruction(Op::SetLocal, &vec![0]),
                        make_instruction(Op::GetGlobal, &vec![0]),
                        make_instruction(Op::GetFree, &vec![0]),
                        make_instruction(Op::Add, &vec![]),
                        make_instruction(Op::GetFree, &vec![1]),
                        make_instruction(Op::Add, &vec![]),
                        make_instruction(Op::GetLocal, &vec![0]),
                        make_instruction(Op::Add, &vec![]),
                        make_instruction(Op::ReturnValue, &vec![]),
                    ]), num_locals: 1, num_parameters: 0})),
                    Object::CompiledFunction(Rc::new(CompiledFunction{instructions: concat_instructions(&vec![
                        make_instruction(Op::Constant, &vec![2]),
                        make_instruction(Op::SetLocal, &vec![0]),
                        make_instruction(Op::GetFree, &vec![0]),
                        make_instruction(Op::GetLocal, &vec![0]),
                        make_instruction(Op::Closure, &vec![4, 2]),
                        make_instruction(Op::ReturnValue, &vec![]),
                    ]), num_locals: 1, num_parameters: 0})),
                    Object::CompiledFunction(Rc::new(CompiledFunction{instructions: concat_instructions(&vec![
                        make_instruction(Op::Constant, &vec![1]),
                        make_instruction(Op::SetLocal, &vec![0]),
                        make_instruction(Op::GetLocal, &vec![0]),
                        make_instruction(Op::Closure, &vec![5, 1]),
                        make_instruction(Op::ReturnValue, &vec![]),
                    ]), num_locals: 1, num_parameters: 0})),
                ],
                expected_instructions: vec![
                    make_instruction(Op::Constant, &vec![0]),
                    make_instruction(Op::SetGobal, &vec![0]),
                    make_instruction(Op::Closure, &vec![6, 0]),
                    make_instruction(Op::Pop, &vec![]),
                ],
            },
        ];

        run_compiler_tests(tests);
    }

    #[test]
    fn resolve_free() {
        let mut global = SymbolTable::new();
        global.define("a");
        global.define("b");

        let mut first_local = SymbolTable::new_enclosed_symbol_table(global.clone());
        first_local.define("c");
        first_local.define("d");

        let mut second_local = SymbolTable::new_enclosed_symbol_table(first_local.clone());
        second_local.define("e");
        second_local.define("f");

        let mut tests = vec![
            (first_local, vec![
                Symbol{name: "a".to_string(), scope: SymbolScope::Global, index: 0},
                Symbol{name: "b".to_string(), scope: SymbolScope::Global, index: 1},
                Symbol{name: "c".to_string(), scope: SymbolScope::Local, index: 0},
                Symbol{name: "d".to_string(), scope: SymbolScope::Local, index: 1},
            ], vec![]),
            (second_local, vec![
                Symbol{name: "a".to_string(), scope: SymbolScope::Global, index: 0},
                Symbol{name: "b".to_string(), scope: SymbolScope::Global, index: 1},
                Symbol{name: "c".to_string(), scope: SymbolScope::Free, index: 0},
                Symbol{name: "d".to_string(), scope: SymbolScope::Free, index: 1},
                Symbol{name: "e".to_string(), scope: SymbolScope::Local, index: 0},
                Symbol{name: "f".to_string(), scope: SymbolScope::Local, index: 1},
            ], vec![
                Symbol{name: "c".to_string(), scope: SymbolScope::Local, index: 0},
                Symbol{name: "d".to_string(), scope: SymbolScope::Local, index: 1},
            ]),
        ];

        for mut t in &mut tests {
            let (table, expected_symbols, expected_free_symbols) = &mut t;

            for exp in expected_symbols.clone() {
                match table.resolve(&exp.name) {
                    Some(got) => assert_eq!(exp, *got),
                    None => panic!("name {} not resolvable", exp.name),
                }
            }

            assert_eq!(table.free_symbols.len(), expected_free_symbols.len());
            let mut i = 0;
            for exp in expected_free_symbols {
                let got = (*table.free_symbols[i]).borrow();
                assert_eq!(*exp, *got);
                i += 1;
            }
        }
    }

    #[test]
    fn resolve_unresolvable_free() {
        let mut global = SymbolTable::new();
        global.define("a");

        let mut first_local = SymbolTable::new_enclosed_symbol_table(global);
        first_local.define("c");

        let mut second_local = SymbolTable::new_enclosed_symbol_table(first_local);
        second_local.define("e");
        second_local.define("f");

        let expected = vec![
            Symbol{name: "a".to_string(), scope: SymbolScope::Global, index: 0},
            Symbol{name: "c".to_string(), scope: SymbolScope::Free, index: 0},
            Symbol{name: "e".to_string(), scope: SymbolScope::Local, index: 0},
            Symbol{name: "f".to_string(), scope: SymbolScope::Local, index: 1},
        ];

        for exp in &expected {
            match second_local.resolve(&exp.name) {
                Some(got) => assert_eq!(exp, got.borrow()),
                None => panic!("name {} not resolvable", exp.name),
            }
        }

        for name in vec!["b", "d"] {
            if let Some(_) = second_local.resolve(name) {
                panic!("name {} resolved but was not expected to", name);
            }
        }
    }

    fn run_compiler_tests(tests: Vec<CompilerTestCase>) {
        for t in tests {
            let program = parse(t.input).unwrap();
            let mut compiler = Compiler::new();
            let bytecode = compiler.compile(program).unwrap_or_else(
                |err| panic!("{} error compiling on input: {}. want: {:?}", err.message, t.input, t.expected_instructions));

            test_instructions(&t.expected_instructions, &bytecode.instructions).unwrap_or_else(
                |err| panic!("{} error on instructions for: {}\nexp: {}\ngot: {}", &err.message, t.input, concat_instructions(&t.expected_instructions).string(), bytecode.instructions.string()));

            test_constants(&t.expected_constants, bytecode.constants.borrow()).unwrap_or_else(
                |err| panic!("{} error on constants for : {}", &err.message, t.input));
        }
    }


    fn test_instructions(expected: &Vec<Instructions>, actual: &Instructions) -> ::std::result::Result<(), CompileError> {
        let concatted = concat_instructions(expected);

        if concatted.len() != actual.len() {
            return Err(CompileError{message: format!("instruction lengths not equal\n\texp:\n{:?}\n\tgot:\n{:?}", concatted.string(), actual.string())})
        }

        let mut pos = 0;

        for (exp, got) in concatted.into_iter().zip(actual) {
            if exp != *got {
                return Err(CompileError { message: format!("exp\n{:?} but got\n{} at position {:?}", exp, got, pos) });
            }
            pos = pos + 1;
        }
        Ok(())
    }

    fn test_constants(expected: &Vec<Object>, actual: &Vec<Rc<Object>>) -> ::std::result::Result<(), CompileError> {
        let mut pos = 0;

        for (exp, got) in expected.into_iter().zip(actual) {
            let got = got.borrow();
            match (exp, got) {
                (Object::Int(exp), Object::Int(got)) => if *exp != *got {
                    return Err(CompileError{message: format!("constant {}, exp: {} got: {}", pos, exp, got)})
                },
                (Object::String(exp), Object::String(got)) => if exp != got {
                    return Err(CompileError{message: format!("constant {}, exp: {} got: {}", pos, exp, got)})
                },
                (Object::CompiledFunction(exp), Object::CompiledFunction(got)) => if exp != got {
                    return Err(CompileError{message: format!("constant {}, exp: {:?} got: {:?}, instructions exp:\n{}\ngot\n{}", pos, exp, got, exp.instructions.string(), got.instructions.string())})
                },
                _ => panic!("can't compare objects: exp: {:?} got: {:?}", exp, got)
            }
            pos = pos + 1;
        }
        Ok(())
    }

    fn concat_instructions(instructions: &Vec<Instructions>) -> Instructions {
        let mut concatted = Instructions::new();

        for i in instructions {
            for u in i {
                concatted.push(*u);
            }
        }

        concatted
    }
}