use crate::compiler::Compiler;
use crate::object::{Object, Array, MonkeyHash, CompiledFunction, Builtin, Closure};
use crate::parser::parse;
use crate::code::{Instructions, Op};
use byteorder;
use self::byteorder::{ByteOrder, BigEndian, ReadBytesExt};
use std::rc::Rc;
use std::borrow::Borrow;
use std::collections::HashMap;

const STACK_SIZE: usize = 2048;
const GLOBAL_SIZE: usize = 65536;
const MAX_FRAMES: usize = 1024;

#[derive(Clone, Debug)]
struct Frame {
    cl: Rc<Closure>,
    ip: usize,
    base_pointer: usize,
}

//impl Frame {
//    fn instructions(&mut self) -> &mut Instructions {
//        &mut self.func.instructions
//    }
//}

pub struct VM<'a> {
    constants: &'a Vec<Rc<Object>>,

    stack: Vec<Rc<Object>>,
    sp: usize,

    pub globals: Vec<Rc<Object>>,

    frames: Vec<Frame>,
    frames_index: usize,
}

impl<'a> VM<'a> {
    pub fn new(constants: &'a Vec<Rc<Object>>, instructions: Instructions) -> VM<'a> {
        let mut stack = Vec::with_capacity(STACK_SIZE);
        stack.resize(STACK_SIZE, Rc::new(Object::Null));

        let mut frames = Vec::with_capacity(MAX_FRAMES);
        let empty_frame = Frame{
            cl: Rc::new(Closure{
                func: Rc::new(CompiledFunction{
                    instructions: vec![],
                    num_locals: 0,
                    num_parameters: 0}),
                free: vec![]}),
            ip: 24,
            base_pointer: 0
        };

        frames.resize(MAX_FRAMES, empty_frame);

        let main_func = Rc::new(CompiledFunction{instructions, num_locals: 0, num_parameters: 0});
        let main_closure = Rc::new(Closure{func: main_func, free: vec![]});
        let main_frame = Frame{cl: main_closure, ip: 0, base_pointer: 0};
        frames[0] = main_frame;

        VM{
            constants,
            stack,
            sp: 0,
            globals: VM::new_globals(),
            frames,
            frames_index: 0,
        }
    }

    pub fn new_globals() -> Vec<Rc<Object>> {
        let mut globals = Vec::with_capacity(GLOBAL_SIZE);
        globals.resize(GLOBAL_SIZE, Rc::new(Object::Null));
        return globals;
    }

    pub fn new_with_global_store(constants: &'a Vec<Rc<Object>>, instructions: Instructions, globals: Vec<Rc<Object>>) -> VM<'a> {
        let mut vm = VM::new(constants, instructions);
        vm.globals = globals;
        return vm;
    }

    pub fn last_popped_stack_elem(&self, ) -> Option<Rc<Object>> {
        match self.stack.get(self.sp) {
            Some(o) => Some(Rc::clone(o)),
            None => None,
        }
    }

    fn continue_current_frame(&self) -> bool {
        let frame = &self.frames[self.frames_index];
        return frame.ip < frame.cl.func.instructions.len();
    }

    fn current_ip(&self) -> usize {
        self.frames[self.frames_index].ip
    }

    fn push_frame(&mut self, frame: Frame) {
        self.frames_index += 1;
        self.frames[self.frames_index] = frame;
    }

    fn pop_frame(&mut self) -> usize {
        self.frames_index -= 1;
        self.frames[self.frames_index + 1].base_pointer
    }

    fn read_op_at(&self, ip: usize) -> Op {
        let ins = &self.frames[self.frames_index].cl.func.instructions;
        unsafe { ::std::mem::transmute(*ins.get_unchecked(ip)) }
    }

    fn read_usize_at(&self, ip: usize) -> usize {
        let ins = &self.frames[self.frames_index].cl.func.instructions;
        BigEndian::read_u16(&ins[ip..ip+2]) as usize
    }

    fn read_u8_at(&self, ip: usize) -> u8 {
        let ins = &self.frames[self.frames_index].cl.func.instructions;
        *&ins[ip]
    }

    fn set_ip(&mut self, ip: usize) {
        self.frames[self.frames_index].ip = ip;
    }

    pub fn run(&mut self) {
        let mut ip = 0;

        while self.continue_current_frame() {
            ip = self.current_ip();
            let op = self.read_op_at(ip);
            self.set_ip(ip + 1);

            match op {
                Op::Constant => {
                    let const_index = self.read_usize_at(ip + 1);
                    self.set_ip(ip + 3);

                    let c = Rc::clone(&self.constants[const_index]);
                    self.push(c)
                },
                Op::Add | Op::Sub | Op::Mul | Op::Div => self.execute_binary_operation(op),
                Op::GreaterThan | Op::Equal | Op::NotEqual => self.execute_comparison(op),
                Op::Pop => {
                    self.pop();
                },
                Op::True => self.push(Rc::new(Object::Bool(true))),
                Op::False => self.push(Rc::new(Object::Bool(false))),
                Op::Bang => self.execute_bang_operator(),
                Op::Minus => self.execute_minus_operator(),
                Op::Jump => {
                    let pos = self.read_usize_at(ip + 1);
                    self.set_ip(pos);
                },
                Op::JumpNotTruthy => {
                    let pos = self.read_usize_at(ip + 1);
                    self.set_ip(ip + 3);

                    let condition = self.pop();
                    if !is_truthy(&condition) {
                        self.set_ip(pos);
                    }
                },
                Op::Null => self.push(Rc::new(Object::Null)),
                Op::SetGobal => {
                    let global_index = self.read_usize_at(ip + 1);
                    self.set_ip(ip + 3);

                    self.globals[global_index] = self.pop();
                },
                Op::SetLocal => {
                    let local_index = self.read_u8_at(ip + 1) as usize;
                    self.set_ip(ip + 2);

                    let base = self.frames[self.frames_index].base_pointer;
                    self.stack[base + local_index] = self.pop();
                },
                Op::GetGlobal => {
                    let global_index = self.read_usize_at(ip + 1);
                    self.set_ip(ip + 3);

                    self.push(Rc::clone(&self.globals[global_index]));
                },
                Op::GetLocal => {
                    let local_index = self.read_u8_at(ip + 1) as usize;
                    self.set_ip(ip + 2);

                    let base = self.frames[self.frames_index].base_pointer;
                    self.push(Rc::clone(&self.stack[base + local_index]));
                }
                Op::Array => {
                    let num_elements = self.read_usize_at(ip + 1);
                    self.set_ip(ip + 3);

                    let array = self.build_array(self.sp - num_elements, self.sp);
                    self.sp -= num_elements;

                    self.push(Rc::new(array));
                },
                Op::Hash => {
                    let num_elements = self.read_usize_at(ip + 1);
                    self.set_ip(ip + 3);

                    let hash = self.build_hash(self.sp - num_elements, self.sp);
                    self.sp -= num_elements;

                    self.push(Rc::new(hash));
                },
                Op::Index => {
                    let index = self.pop();
                    let left = self.pop();

                    self.execute_index_expression(left, index);
                },
                Op::Call => {
                    let num_args = self.read_u8_at(ip + 1) as usize;
                    self.set_ip(ip + 2);
                    self.execute_call(num_args);
                },
                Op::ReturnValue => {
                    let return_value = self.pop();
                    let base_pointer = self.pop_frame();
                    self.sp = base_pointer - 1;
                    self.push(return_value);
                },
                Op::Return => {
                    let base_pointer = self.pop_frame();
                    self.sp = base_pointer - 1;
                    self.push(Rc::new(Object::Null));
                },
                Op::GetBuiltin => {
                    let builtin_index = self.read_u8_at(ip + 1);
                    let builtin: Builtin = unsafe { ::std::mem::transmute(builtin_index) };
                    self.set_ip(ip + 2);
                    self.push(Rc::new(Object::Builtin(builtin)));
                },
                Op::Closure => {
                    let const_index = self.read_usize_at(ip + 1);
                    let num_free = self.read_u8_at(ip + 3) as usize;
                    self.set_ip(ip + 4);
                    self.push_closure(const_index, num_free);
                },
                Op::GetFree => {
                    let free_index = self.read_u8_at(ip + 1) as usize;
                    self.set_ip(ip + 2);
                    let obj = &self.frames[self.frames_index].cl.free[free_index];
                    self.push(Rc::clone(obj));
                },
                _ => panic!("unsupported op {:?}", op),
            }
        }
    }

    fn push_closure(&mut self, const_index: usize, num_free: usize) {
        match &*self.constants[const_index] {
            Object::CompiledFunction(func) => {
                let mut free = Vec::with_capacity(num_free);
                for obj in &self.stack[self.sp-num_free..self.sp] {
                    free.push(Rc::clone(obj));
                }
                self.sp = self.sp-num_free;
                self.push(Rc::new(Object::Closure(Rc::new(Closure{func: Rc::clone(&func), free}))));
            },
            obj => panic!("not a function: {:?}", obj),
        }

    }

    fn execute_call(&mut self, num_args: usize) {
        if let Some(frame) = match &*self.stack[self.sp - 1 - num_args] {
            Object::Closure(ref cl) => {
                Some(Frame{cl: Rc::clone(cl), ip: 0, base_pointer: self.sp - num_args})
            },
            _ => None,
        } { self.call_closure(frame, num_args); } else if let Some(builtin) = match &*self.stack[self.sp - 1 - num_args] {
            Object::Builtin(builtin) => Some(builtin),
            _ => None,
        } { self.call_builtin(*builtin, num_args) } else {
            panic!("called non-function {:?}", self.stack[self.sp - 1 - num_args]);
        }
    }

    fn call_closure(&mut self, frame: Frame, num_args: usize) {
        if num_args != frame.cl.func.num_parameters {
            panic!("function expects {} arguments but got {}", frame.cl.func.num_parameters, num_args);
        }

        let sp = frame.base_pointer + frame.cl.func.num_locals;
        self.push_frame(frame);
        self.sp = sp;
    }

    fn call_builtin(&mut self, builtin: Builtin, num_args: usize) {
        let args = &self.stack[self.sp - num_args..self.sp].to_vec();
        let result = builtin.apply(args);
        self.sp = self.sp - num_args - 1;

        match result {
            Ok(obj) => self.push(obj),
            Err(err) => panic!("error calling builtin: {:?}", err),
        }
    }

    fn execute_index_expression(&mut self, left: Rc<Object>, index: Rc<Object>) {
        match (&*left, &*index) {
            (Object::Array(arr), Object::Int(idx)) => self.execute_array_index(&arr, *idx),
            (Object::Hash(hash), _) => self.execute_hash_index(hash, index),
            _ => panic!("index not supported on: {:?} {:?}", left, index),
        }
    }

    fn execute_array_index(&mut self, arr: &Rc<Array>, idx: i64) {
        match arr.elements.get(idx as usize) {
            Some(el) => self.push(Rc::clone(el)),
            None => self.push(Rc::new(Object::Null)),
        }
    }

    fn execute_hash_index(&mut self, hash: &Rc<MonkeyHash>, index: Rc<Object>) {
        match &*index {
            Object::String(_) | Object::Int(_) | Object::Bool(_) => {
                match hash.pairs.get(&*index) {
                    Some(obj) => self.push(Rc::clone(obj)),
                    None => self.push(Rc::new(Object::Null)),
                }
            },
            _ => panic!("unusable as hash key: {}", index)
        }
    }

    fn build_array(&mut self, start_index: usize, end_index: usize) -> Object {
        let mut elements = Vec::with_capacity(end_index - start_index);
        elements.resize(end_index - start_index, Rc::new(Object::Null));
        let mut i = start_index;

        while i < end_index {
            elements[i-start_index] = self.stack[i].clone();
            i += 1;
        }

        Object::Array(Rc::new(Array{elements}))
    }

    fn build_hash(&mut self, start_index: usize, end_index: usize) -> Object {
        let mut hash = HashMap::new();
        let mut i = start_index;

        while i < end_index {
            let key = self.stack[i].clone();
            let value = self.stack[i + 1].clone();

            hash.insert(key, value);
            i += 2;
        }

        Object::Hash(Rc::new(MonkeyHash{pairs: hash}))
    }

    fn execute_binary_operation(&mut self, op: Op) {
        let right = self.pop();
        let left = self.pop();

        match (left.borrow(), right.borrow()) {
            (Object::Int(left), Object::Int(right)) => {
                let result = match op {
                    Op::Add => left + right,
                    Op::Sub => left - right,
                    Op::Mul => left * right,
                    Op::Div => left / right,
                    _ => panic!("unsupported operator in integer binary operation {:?}", op)
                };

                self.push(Rc::new(Object::Int(result)));
            },
            (Object::String(left), Object::String(right)) => {
                let mut result = left.clone();
                match op {
                    Op::Add => result.push_str(&right),
                    _ => panic!("unsupported operator in string binary operation {:?}", op)
                };

                self.push(Rc::new(Object::String(result)));
            },
            _ => panic!("unable to {:?} {:?} and {:?}", op, left, right),
        }
    }

    fn execute_comparison(&mut self, op: Op) {
        let right = self.pop();
        let left = self.pop();

        match (left.borrow(), right.borrow()) {
            (Object::Int(left), Object::Int(right)) => {
                let result = match op {
                    Op::Equal => left == right,
                    Op::NotEqual => left != right,
                    Op::GreaterThan => left > right,
                    _ => panic!("unsupported operator in comparison {:?}", op)
                };

                self.push(Rc::new(Object::Bool(result)));
            },
            (Object::Bool(left), Object::Bool(right)) => {
                let result = match op {
                    Op::Equal => left == right,
                    Op::NotEqual => left != right,
                    _ => panic!("unsupported operator in comparison {:?}", op)
                };

                self.push(Rc::new(Object::Bool(result)));
            },
            _ => panic!("unable to {:?} {:?} and {:?}", op, left, right),
        }
    }

    fn execute_bang_operator(&mut self) {
        let op = self.pop();

        match op.borrow() {
            Object::Bool(true) => self.push(Rc::new(Object::Bool(false))),
            Object::Bool(false) => self.push(Rc::new(Object::Bool(true))),
            Object::Null => self.push(Rc::new(Object::Bool(true))),
            _ => self.push(Rc::new(Object::Bool(false))),
        }
    }

    fn execute_minus_operator(&mut self) {
        let op = self.pop();

        match op.borrow() {
            Object::Int(int) => self.push(Rc::new(Object::Int(-*int))),
            _ => panic!("unsupported type for negation {:?}", op)
        }
    }

    fn push(&mut self, o: Rc<Object>) {
        if self.sp >= STACK_SIZE {
            panic!("stack overflow")
        }

        self.stack[self.sp] = o;
        self.sp += 1;
    }

    fn pop(&mut self) -> Rc<Object> {
        self.sp -= 1;
        Rc::clone(&self.stack[self.sp])
    }
}

fn is_truthy(obj: &Rc<Object>) -> bool {
    match obj.borrow() {
        Object::Bool(v) => *v,
        Object::Null => false,
        _ => true,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::rc::Rc;
    use std::collections::HashMap;

    struct VMTestCase<'a> {
        input: &'a str,
        expected: Object,
    }

    #[test]
    fn integer_arithmetic() {
        let tests = vec![
            VMTestCase{input: "1", expected: Object::Int(1)},
            VMTestCase{input: "2", expected: Object::Int(2)},
            VMTestCase{input: "1 + 2", expected: Object::Int(3)},
            VMTestCase{input: "1 - 2", expected: Object::Int(-1)},
            VMTestCase{input: "1 * 2", expected: Object::Int(2)},
            VMTestCase{input: "4 / 2", expected: Object::Int(2)},
            VMTestCase{input: "50 / 2 * 2 + 10 - 5", expected: Object::Int(55)},
            VMTestCase{input: "5 * (2 + 10)", expected: Object::Int(60)},
            VMTestCase{input: "5 + 5 + 5 + 5 - 10", expected: Object::Int(10)},
            VMTestCase{input: "2 * 2 * 2 * 2 * 2", expected: Object::Int(32)},
            VMTestCase{input: "5 * 2 + 10", expected: Object::Int(20)},
            VMTestCase{input: "5 + 2 * 10", expected: Object::Int(25)},
            VMTestCase{input: "5 * (2 + 10)", expected: Object::Int(60)},
            VMTestCase{input: "-5", expected: Object::Int(-5)},
            VMTestCase{input: "-10", expected: Object::Int(-10)},
            VMTestCase{input: "-50 + 100 + -50", expected: Object::Int(0)},
            VMTestCase{input: "(5 + 10 * 2 + 15 / 3) * 2 + -10", expected: Object::Int(50)},
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn boolean_expressions() {
        let tests = vec![
            VMTestCase{input: "true", expected: Object::Bool(true)},
            VMTestCase{input: "false", expected: Object::Bool(false)},
            VMTestCase{input: "1 < 2", expected: Object::Bool(true)},
            VMTestCase{input: "1 > 2", expected: Object::Bool(false)},
            VMTestCase{input: "1 < 1", expected: Object::Bool(false)},
            VMTestCase{input: "1 > 1", expected: Object::Bool(false)},
            VMTestCase{input: "1 == 1", expected: Object::Bool(true)},
            VMTestCase{input: "1 != 1", expected: Object::Bool(false)},
            VMTestCase{input: "1 == 2", expected: Object::Bool(false)},
            VMTestCase{input: "1 != 2", expected: Object::Bool(true)},
            VMTestCase{input: "true == true", expected: Object::Bool(true)},
            VMTestCase{input: "false == false", expected: Object::Bool(true)},
            VMTestCase{input: "true == false", expected: Object::Bool(false)},
            VMTestCase{input: "true != false", expected: Object::Bool(true)},
            VMTestCase{input: "false != true", expected: Object::Bool(true)},
            VMTestCase{input: "(1 < 2) == true", expected: Object::Bool(true)},
            VMTestCase{input: "(1 < 2) == false", expected: Object::Bool(false)},
            VMTestCase{input: "(1 > 2) == true", expected: Object::Bool(false)},
            VMTestCase{input: "(1 > 2) == false", expected: Object::Bool(true)},
            VMTestCase{input: "!true", expected: Object::Bool(false)},
            VMTestCase{input: "!false", expected: Object::Bool(true)},
            VMTestCase{input: "!5", expected: Object::Bool(false)},
            VMTestCase{input: "!!true", expected: Object::Bool(true)},
            VMTestCase{input: "!!false", expected: Object::Bool(false)},
            VMTestCase{input: "!!5", expected: Object::Bool(true)},
            VMTestCase{input: "!(if (false) { 5; })", expected: Object::Bool(true)},
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn conditionals() {
        let tests = vec![
            VMTestCase{input: "if (true) { 10 }", expected: Object::Int(10)},
            VMTestCase{input: "if (true) { 10 } else { 20 }", expected: Object::Int(10)},
            VMTestCase{input: "if (false) { 10 } else { 20 }", expected: Object::Int(20)},
            VMTestCase{input: "if (1) { 10 }", expected: Object::Int(10)},
            VMTestCase{input: "if (1 < 2) { 10 }", expected: Object::Int(10)},
            VMTestCase{input: "if (1 < 2) { 10 } else { 20 }", expected: Object::Int(10)},
            VMTestCase{input: "if (1 > 2) { 10 } else { 20 }", expected: Object::Int(20)},
            VMTestCase{input: "if (1 > 2) { 10 }", expected: Object::Null},
            VMTestCase{input: "if (false) { 10 }", expected: Object::Null},
            VMTestCase{input: "if ((if (false) { 10 })) { 10 } else { 20 }", expected: Object::Int(20)},
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn global_let_statements() {
        let tests = vec![
            VMTestCase{input: "let one = 1; one", expected: Object::Int(1)},
            VMTestCase{input: "let one = 1; let two = 2; one + two", expected: Object::Int(3)},
            VMTestCase{input: "let one = 1; let two = one + one; one + two", expected: Object::Int(3)},
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn string_expressions() {
        let tests = vec![
            VMTestCase{input: "\"monkey\"", expected: Object::String("monkey".to_string())},
            VMTestCase{input: "\"mon\" + \"key\"", expected: Object::String("monkey".to_string())},
            VMTestCase{input: "\"mon\" + \"key\" + \"banana\"", expected: Object::String("monkeybanana".to_string())},
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn array_literals() {
        let tests = vec![
            VMTestCase{
                input: "[]",
                expected: Object::Array(Rc::new(Array{
                    elements: vec![],
                })),
            },
            VMTestCase{
                input: "[1, 2, 3]",
                expected: Object::Array(Rc::new(Array{
                    elements: vec![
                        Rc::new(Object::Int(1)),
                        Rc::new(Object::Int(2)),
                        Rc::new(Object::Int(3)),
                    ],
                })),
            },
            VMTestCase{
                input: "[1 + 2, 3 * 4, 5 + 6]",
                expected: Object::Array(Rc::new(Array{
                    elements: vec![
                        Rc::new(Object::Int(3)),
                        Rc::new(Object::Int(12)),
                        Rc::new(Object::Int(11)),
                    ],
                })),
            },
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn hash_literals() {
        macro_rules! map(
            { $($key:expr => $value:expr),+ } => {
                {
                    let mut m = ::std::collections::HashMap::new();
                    $(
                        m.insert($key, $value);
                    )+
                    m
                }
            };
        );

        let tests = vec![
            VMTestCase{
                input: "{}",
                expected: hash_to_object(HashMap::new()),
            },
            VMTestCase{
                input: "{1: 2, 2: 3}",
                expected: hash_to_object(map!{1 => 2, 2 => 3}),
            },
            VMTestCase{
                input: "{1 + 1: 2 * 2, 3 + 3: 4 * 4}",
                expected: hash_to_object(map!{2 => 4, 6 => 16}),
            },
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn index_expressions() {
        let tests = vec![
            VMTestCase{input: "[1, 2, 3][1]", expected: Object::Int(2)},
            VMTestCase{input: "[1, 2, 3][0 + 2]", expected: Object::Int(3)},
            VMTestCase{input: "[[1, 1, 1]][0][0]", expected: Object::Int(1)},
            VMTestCase{input: "[][0]", expected: Object::Null},
            VMTestCase{input: "[1, 2, 3][99]", expected: Object::Null},
            VMTestCase{input: "[1][-1]", expected: Object::Null},
            VMTestCase{input: "{1: 1, 2: 2}[1]", expected: Object::Int(1)},
            VMTestCase{input: "{1: 1, 2: 2}[2]", expected: Object::Int(2)},
            VMTestCase{input: "{1: 1}[0]", expected: Object::Null},
            VMTestCase{input: "{}[0]", expected: Object::Null},
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn calling_functions_without_arguments() {
        let tests = vec![
            VMTestCase{
                input: "let fivePlusTen= fn() { 5 + 10; }; fivePlusTen();",
                expected: Object::Int(15)
            },
            VMTestCase{
                input: "let one = fn() { 1; }; let two = fn() { 2; }; one() + two();",
                expected: Object::Int(3),
            },
            VMTestCase{
                input: "let a = fn() { 1 }; let b = fn() { a() + 1 }; let c = fn() { b() + 1 }; c();",
                expected: Object::Int(3),
            },
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn functions_with_return_statement() {
        let tests = vec![
            VMTestCase{
                input: "let earlyExit = fn() { return 99; 100; }; earlyExit();",
                expected: Object::Int(99),
            },
            VMTestCase{
                input: "let earlyExit = fn() { return 99; return 100; }; earlyExit();",
                expected: Object::Int(99),
            }
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn functions_without_return_value() {
        let tests = vec![
            VMTestCase{
                input: "let noReturn = fn() { }; noReturn();",
                expected: Object::Null,
            },
            VMTestCase{
                input: "let noReturn = fn() { }; let noReturnTwo = fn() { noReturn(); }; noReturn(); noReturnTwo();",
                expected: Object::Null,
            },
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn first_class_functions() {
        let tests = vec![
            VMTestCase{
                input: "let returnsOne = fn() { 1; }; let returnsOneReturner = fn() { returnsOne; }; returnsOneReturner()();",
                expected: Object::Int(1),
            }
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn calling_functions_with_bindings() {
        let tests = vec![
            VMTestCase{
                input: "let one = fn() { let one = 1; one }; one();",
                expected: Object::Int(1),
            },
            VMTestCase{
                input: "let oneAndTwo = fn() { let one = 1; let two = 2; one + two; }; oneAndTwo();",
                expected: Object::Int(3),
            },
            VMTestCase{
                input: "let oneAndTwo = fn() { let one = 1; let two = 2; one + two; };
        let threeAndFour = fn() { let three = 3; let four = 4; three + four; };
        oneAndTwo() + threeAndFour();",
                expected: Object::Int(10),
            },
            VMTestCase{
                input: "let firstFoobar = fn() { let foobar = 50; foobar; };
        let secondFoobar = fn() { let foobar = 100; foobar; };
        firstFoobar() + secondFoobar();",
                expected: Object::Int(150),
            },
            VMTestCase{
                input: "let globalSeed = 50;
        let minusOne = fn() {
            let num = 1;
            globalSeed - num;
        }
        let minusTwo = fn() {
            let num = 2;
            globalSeed - num;
        }
        minusOne() + minusTwo();",
                expected: Object::Int(97),
            }
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn calling_functions_with_arguments_and_bindings() {
        let tests = vec![
            VMTestCase{
                input: "let identity = fn(a) { a; }; identity(4);",
                expected: Object::Int(4),
            },
            VMTestCase{
                input: "let sum = fn(a, b) { a + b; }; sum(1, 2);",
                expected: Object::Int(3),
            },
            VMTestCase{
                input: "let sum = fn(a, b) {
                            let c = a + b;
                            c;
                        };
                        sum(1, 2);",
                expected: Object::Int(3),
            },
            VMTestCase{
                input: "let sum = fn(a, b) {
                            let c = a + b;
                            c;
                        };
                        sum(1, 2) + sum(3, 4);",
                expected: Object::Int(10),
            },
            VMTestCase{
                input: "let sum = fn(a, b) {
                            let c = a + b;
                            c;
                        };
                        let outer = fn() {
                            sum(1, 2) + sum(3, 4);
                        };
                        outer();",
                expected: Object::Int(10),
            },
            VMTestCase{
                input: "let globalNum = 10;

                        let sum = fn(a, b) {
                            let c = a + b;
                            c + globalNum;
                        };

                        let outer = fn() {
                            sum(1, 2) + sum(3, 4) + globalNum;
                        };

                        outer() + globalNum;",
                expected: Object::Int(50),
            }
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn builtin_functions() {
        let tests = vec![
            VMTestCase{input: r#"len("")"#, expected: Object::Int(0)},
            VMTestCase{input: r#"len("four")"#, expected: Object::Int(4)},
            VMTestCase{input: r#"len("hello world")"#, expected: Object::Int(11)},
            VMTestCase{input: "len([1, 2, 3])", expected: Object::Int(3)},
            VMTestCase{input: "len([])", expected: Object::Int(0)},
            VMTestCase{input: r#"puts("hello", "world!")"#, expected: Object::Null},
            VMTestCase{input: "first([1, 2, 3])", expected: Object::Int(1)},
            VMTestCase{input: "first([])", expected: Object::Null},
            VMTestCase{input: "last([1, 2, 3])", expected: Object::Int(3)},
            VMTestCase{input: "last([])", expected: Object::Null},
            VMTestCase{input: "rest([1, 2, 3])", expected: Object::Array(Rc::new(Array{elements: vec![Rc::new(Object::Int(2)), Rc::new(Object::Int(3))]}))},
            VMTestCase{input: "rest([])", expected: Object::Array(Rc::new(Array{elements: vec![]}))},
            VMTestCase{input: "push([], 1)", expected: Object::Array(Rc::new(Array{elements: vec![Rc::new(Object::Int(1))]}))},
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn closures() {
        let tests = vec![
            VMTestCase{
                input: "
                    let newClosure = fn(a) {
                        fn() { a; };
                    };
                    let closure = newClosure(99);
                    closure();",
                expected: Object::Int(99),
            },
            VMTestCase{
                input: "
                    let newAdder = fn(a, b) {
                        fn(c) { a + b + c };
                    };
                    let adder = newAdder(1, 2);
                    adder(8);",
                expected: Object::Int(11),
            },
            VMTestCase{
                input: "
                    let newAdder = fn(a, b) {
                        let c = a + b;
                        fn(d) { c + d };
                    };
                    let adder = newAdder(1, 2);
                    adder(8);",
                expected: Object::Int(11),
            },
            VMTestCase{
                input: "
                    let newAdderOuter = fn(a, b) {
                        let c = a + b;
                        fn(d) {
                            let e = d + c;
                            fn(f) { e + f; };
                        };
                    };
                    let newAdderInner = newAdderOuter(1, 2)
                    let adder = newAdderInner(3);
                    adder(8);",
                expected: Object::Int(14),
            }
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn recursive_fibonacci() {
        let tests = vec![
            VMTestCase{
                input: "
                    let fibonacci = fn(x) {
                        if (x == 0) {
                            return 0;
                        } else {
                            if (x == 1) {
                                return 1;
                            } else {
                                fibonacci(x - 1) + fibonacci(x - 2);
                            }
                        }
                    };
                    fibonacci(15);",
                expected: Object::Int(610),
            },
        ];

        run_vm_tests(tests);
    }

    // commented this out because the should_panic macro doesn't work with the CLION test runner
//    #[test]
//    #[should_panic(expected = "function expects 0 arguments but got 1")]
//    fn calling_functions_with_wrong_arguments() {
//        let tests = vec![
//            VMTestCase{
//                input: "fn() { 1; }(1);",
//                expected: Object::String("asdf".to_string()),
//            },
//        ];
//
//        run_vm_tests(tests);
//    }

    fn hash_to_object(h: HashMap<i64,i64>) -> Object {
        let hash = HashMap::new();
        let mut mh = MonkeyHash{pairs: hash};

        for (h, k) in h {
            mh.pairs.insert(Rc::new(Object::Int(h)), Rc::new(Object::Int(k)));
        }

        Object::Hash(Rc::new(mh))
    }

    fn run_vm_tests(tests: Vec<VMTestCase>) {
        for t in tests {
            let program = parse(t.input).unwrap();
            let mut compiler = Compiler::new();
            let bytecode = compiler.compile(program).unwrap();

            let mut vm = VM::new(bytecode.constants, bytecode.instructions.to_vec());

            vm.run();

            let got = vm.last_popped_stack_elem();
            test_object(&t.expected, got.unwrap().borrow());
        }
    }

    fn test_object(exp: &Object, got: &Object) {
        match (&exp, &got) {
            (Object::Int(exp), Object::Int(got)) => if exp != got {
                panic!("ints not equal: exp: {}, got: {}", exp, got)
            },
            (Object::Bool(exp), Object::Bool(got)) => if exp != got {
                panic!("bools not equal: exp: {}, got: {}", exp, got)
            },
            (Object::String(exp), Object::String(got)) => if exp != got {
                panic!("strings not equal: exp: {}, got: {}", exp, got)
            },
            (Object::Array(exp), Object::Array(got)) => if exp != got {
                panic!("arrays not equal: exp: {:?}, got: {:?}", exp, got)
            },
            (Object::Hash(exp), Object::Hash(got)) => if exp != got {
                panic!("hashes not equal: exp: {:?}, got: {:?}", exp, got)
            },
            (Object::Null, Object::Null) => (),
            _ => panic!("can't compare objects: exp: {:?}, got: {:?}", exp, got)
        }
    }
}