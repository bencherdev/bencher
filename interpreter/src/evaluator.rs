use std;
use std::fmt;
use std::cell::RefCell;
use std::rc::Rc;
use std::collections::HashMap;
use crate::ast::*;
use crate::object;
use crate::object::{Object, Environment, Function, Builtin, Array, MonkeyHash};
use crate::token::Token;
use crate::parser;

pub type EvalResult = Result<Rc<Object>, EvalError>;

#[derive(Debug)]
pub struct EvalError {
    pub message: String,
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for EvalError {
    fn description(&self) -> &str {
        &self.message
    }
}

pub fn eval(node: &Node, env: Rc<RefCell<Environment>>) -> EvalResult {
    match node {
        Node::Program(prog) => eval_program(&prog, env),
        Node::Statement(stmt) => eval_statement(&stmt, env),
        Node::Expression(exp) => eval_expression(&exp, env),
    }
}

fn eval_expression(exp: &Expression, env: Rc<RefCell<Environment>>) -> EvalResult {
    match exp {
        Expression::Integer(int) => Ok(Rc::new(Object::Int(*int))),
        Expression::Boolean(b) => Ok(Rc::new(Object::Bool(*b))),
        Expression::String(s) => Ok(Rc::new(Object::String(s.clone()))),
        Expression::Prefix(pre) => {
            let right = eval_expression(&pre.right, env)?;
            eval_prefix_expression(&pre.operator, right)
        },
        Expression::Infix(infix) => {
            let left = eval_expression(&infix.left, Rc::clone(&env))?;
            let right = eval_expression(&infix.right, env)?;
            eval_infix_expression(&infix.operator, left, right)
        },
        Expression::If(ifexp) => {
            let evaluated = eval_expression(&ifexp.condition, Rc::clone(&env))?;

            match is_truthy(&evaluated) {
                true => eval_block(&ifexp.consequence, env),
                false  => {
                    match &ifexp.alternative {
                        Some(alt) => eval_block(&alt, env),
                        None => Ok(Rc::new(Object::Null)),
                    }
                }
            }
        },
        Expression::Identifier(ident) => {
            eval_identifier(ident, env)
        },
        Expression::Function(f) => {
            let func = Function{parameters: f.parameters.clone(), body: f.body.clone(), env: Rc::clone(&env)};
            Ok(Rc::new(Object::Function(Rc::new(func))))
        },
        Expression::Call(exp) => {
            let function = eval_expression(&exp.function, Rc::clone(&env))?;
            let args = eval_expressions(&exp.arguments, env)?;
            apply_function(&function, &args)
        },
        Expression::Array(a) => {
            let elements = eval_expressions(&a.elements, Rc::clone(&env))?;
            Ok(Rc::new(Object::Array(Rc::new(Array{elements}))))
        },
        Expression::Index(i) => {
            let left = eval_expression(&i.left, Rc::clone(&env))?;
            let index = eval_expression(&i.index, env)?;
            eval_index_expression(left, index)
        },
        Expression::Hash(h) => {
            eval_hash_literal(&h, Rc::clone(&env))
        },
    }
}

fn eval_hash_literal(h: &HashLiteral, env: Rc<RefCell<Environment>>) -> EvalResult {
    let mut pairs = HashMap::new();

    for (key_exp, val_exp) in &h.pairs {
        let key = eval_expression(key_exp, Rc::clone(&env))?;
        let value = eval_expression(val_exp, Rc::clone(&env))?;
        pairs.insert(key, value);
    }

    Ok(Rc::new(Object::Hash(Rc::new(MonkeyHash{pairs}))))
}

fn eval_index_expression(left: Rc<Object>, index: Rc<Object>) -> EvalResult {
    match (&*left, &*index) {
        (Object::Array(a), Object::Int(i)) => {
            match a.elements.get(*i as usize) {
                Some(el) => Ok(Rc::clone(el)),
                None => Ok(Rc::new(Object::Null))
            }
        },
        (Object::Hash(h), _object) => {
            match &*index {
                Object::String(_) | Object::Int(_) | Object::Bool(_) => {
                    match h.pairs.get(&*index) {
                        Some(obj) => Ok(Rc::clone(obj)),
                        None => Ok(Rc::new(Object::Null))
                    }
                },
                _ => Err(EvalError{message: format!("unusable as hash key: {}", index)})
            }
        }
        _ => Err(EvalError{message: format!("index operator not supported {}", index)})
    }
}

fn eval_identifier(ident: &str, env: Rc<RefCell<Environment>>) -> EvalResult {
    match env.borrow().get(ident) {
        Some(obj) => {
            Ok(obj.clone())
        },
        None => {
            match Builtin::lookup(ident) {
                Some(obj) => Ok(Rc::new(obj)),
                None => Err(EvalError{message: format!("identifier not found: {}", ident)}),
            }
        }
    }
}

fn apply_function(func: &Object, args: &Vec<Rc<Object>>) -> EvalResult {
    match func {
        Object::Function(f) => {
            let extended_env = extend_function_env(f, args);
            let evaluated = eval_block(&f.body, extended_env)?;
            Ok(unwrap_return_value(evaluated))
        },
        Object::Builtin(b) => {
            match b.apply(args) {
                Ok(obj) => Ok(obj),
                Err(err) => Err(EvalError{message: err}),
            }
        },
        f => Err(EvalError{message: format!("{:?} is not a function", f)})
    }
}

fn extend_function_env(func: &Function, args: &Vec<Rc<Object>>) -> Rc<RefCell<Environment>> {
    let env = Rc::new(RefCell::new(Environment::new_enclosed(Rc::clone(&func.env))));

    let mut args_iter = args.into_iter();

    for param in &func.parameters {
        let arg = args_iter.next().unwrap();

        env.borrow_mut().set(param.name.clone(), Rc::clone(arg))
    }

    env
}

fn unwrap_return_value(obj: Rc<Object>) -> Rc<Object> {
    if let Object::Return(ret) = &*obj {
        return Rc::clone(&ret.value)
    }
    obj
}

fn eval_expressions(exps: &Vec<Expression>, env: Rc<RefCell<Environment>>) -> Result<Vec<Rc<Object>>, EvalError> {
    let mut objs = Vec::with_capacity(exps.len());

    for e in exps {
        let res = eval_expression(&e, Rc::clone(&env))?;
        objs.push(res);
    }

    Ok(objs)
}

fn is_truthy(obj: &Object) -> bool {
    match obj {
        Object::Null => false,
        Object::Bool(false) => false,
        _ => true,
    }
}

fn eval_infix_expression(operator: &Token, left: Rc<Object>, right: Rc<Object>) -> EvalResult {
    match (&*left, &*right) {
        (Object::Int(l), Object::Int(r)) => eval_integer_infix_expression(operator, *l, *r),
        (Object::Bool(l), Object::Bool(r)) => eval_bool_infix_expression(operator, *l, *r),
        (Object::String(l), Object::String(r)) => eval_string_infix_expression(operator, l.clone(), &*r),
        _ => Err(EvalError{message: format!("type mismatch: {:?} {} {:?}", left, operator, right)}),
    }
}

fn eval_string_infix_expression(operator: &Token, left: String, right: &str) -> EvalResult {
    match operator {
        Token::Plus => Ok(Rc::new(Object::String(left + right))),
        _ => Err(EvalError{message: format!("unknown operator {} {} {}", left, operator, right)}),
    }
}

fn eval_bool_infix_expression(operator: &Token, left: bool, right: bool) -> EvalResult {
    match operator {
        Token::Eq => Ok(Rc::new(Object::Bool(left == right))),
        Token::Neq => Ok(Rc::new(Object::Bool(left != right))),
        _ => Err(EvalError{message: format!("unknown operator: {} {} {}", left, operator, right)})
    }
}

fn eval_integer_infix_expression(operator: &Token, left: i64, right: i64) -> EvalResult {
    match operator {
        Token::Plus => Ok(Rc::new(Object::Int(left + right))),
        Token::Minus => Ok(Rc::new(Object::Int(left - right))),
        Token::Asterisk => Ok(Rc::new(Object::Int(left * right))),
        Token::Slash => Ok(Rc::new(Object::Int(left / right))),
        Token::Lt => Ok(Rc::new(Object::Bool(left < right))),
        Token::Gt => Ok(Rc::new(Object::Bool(left > right))),
        Token::Eq => Ok(Rc::new(Object::Bool(left == right))),
        Token::Neq => Ok(Rc::new(Object::Bool(left != right))),
        _ => Err(EvalError{message: format!("unknown operator {}", operator)}),
    }
}

fn eval_block(block: &BlockStatement, env: Rc<RefCell<Environment>>) -> EvalResult {
    let mut result = Rc::new(Object::Null);

    for stmt in &block.statements {
        let res = eval_statement(stmt, Rc::clone(&env))?;

        match *res {
            Object::Return(_) => return Ok(res),
            _ => result = res,
        }
    }

    Ok(result)
}

fn eval_statement(stmt: &Statement, env: Rc<RefCell<Environment>>) -> EvalResult {
    match stmt {
        Statement::Expression(exp) => eval_expression(&exp.expression, env),
        Statement::Return(ret) => {
            let value = eval_expression(&ret.value, env)?;
            Ok(Rc::new(Object::Return(Rc::new(object::Return{value}))))
        },
        Statement::Let(stmt) => {
            let exp = eval_expression(&stmt.value, Rc::clone(&env))?;
            let obj = Rc::clone(&exp);
            env.borrow_mut().set(stmt.name.clone(), obj);
            Ok(exp)
        }
    }
}

fn eval_program(prog: &Program, env: Rc<RefCell<Environment>>) -> EvalResult {
    let mut result = Rc::new(Object::Null);

    for stmt in &prog.statements {
        let res = eval_statement(stmt, Rc::clone(&env))?;
        let v = Rc::clone(&res);

        match &*v {
            Object::Return(r) => return Ok(Rc::clone(&r.value)),
            _ => result = res,
        }
    }

    Ok(result)
}

fn eval_prefix_expression(operator: &Token, right: Rc<Object>) -> EvalResult {
    match *operator {
        Token::Bang => eval_bang_operator_expression(right),
        Token::Minus => eval_minus_prefix_operator_expression(right),
        _ => Err(EvalError{message:format!("unknown prefix operator {}", operator)}),
    }
}

fn eval_bang_operator_expression(right: Rc<Object>) -> EvalResult {
    Ok(Rc::new(match *right {
        Object::Bool(true) => Object::Bool(false),
        Object::Bool(false) => Object::Bool(true),
        Object::Null => Object::Bool(true),
        _ => Object::Bool(false),
    }))
}

fn eval_minus_prefix_operator_expression(right: Rc<Object>) -> EvalResult {
    match *right {
        Object::Int(val) => {
            Ok(Rc::new(Object::Int(-val)))
        },
        _ => Err(EvalError{message: format!("unknown operator: -{:?}", right)}),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::object::Object;

    #[test]
    fn eval_integer_expression() {
        struct Test<'a> {
            input: &'a str,
            expected: i64,
        }
        let tests = vec![
            Test{input: "5", expected: 5},
            Test{input: "10", expected: 10},
            Test{input: "-5", expected: -5},
            Test{input: "-10", expected: -10},
            Test{input: "5 + 5 + 5 + 5 - 10", expected: 10},
            Test{input: "2 * 2 * 2 * 2 * 2", expected: 32},
            Test{input: "-50 + 100 + -50", expected: 0},
            Test{input: "5 * 2 + 10", expected: 20},
            Test{input: "5 + 2 * 10", expected: 25},
            Test{input: "20 + 2 * -10", expected: 0},
            Test{input: "50 / 2 * 2 + 10", expected: 60},
            Test{input: "2 * (5 + 10)", expected: 30},
            Test{input: "3 * 3 * 3 + 10", expected: 37},
            Test{input: "3 * (3 * 3) + 10", expected: 37},
            Test{input: "(5 + 10 * 2 + 15 / 3) * 2 + -10", expected: 50},
        ];

        for t in tests {
            let evaluated = test_eval(t.input);
            test_integer_object(&evaluated, t.expected);
        }
    }

    #[test]
    fn eval_boolean_expression() {
        struct Test<'a> {
            input: &'a str,
            expected: bool,
        }
        let tests = vec![
            Test{input: "true", expected: true},
            Test{input: "false", expected: false},
            Test{input: "1 < 2", expected: true},
            Test{input: "1 > 2", expected: false},
            Test{input: "1 < 1", expected: false},
            Test{input: "1 > 1", expected: false},
            Test{input: "1 == 1", expected: true},
            Test{input: "1 != 1", expected: false},
            Test{input: "1 == 2", expected: false},
            Test{input: "1 != 2", expected: true},
            Test{input: "true == true", expected: true},
            Test{input: "false == false", expected: true},
            Test{input: "true == false", expected: false},
            Test{input: "true != false", expected: true},
            Test{input: "false != true", expected: true},
            Test{input: "(1 < 2) == true", expected: true},
            Test{input: "(1 < 2) == false", expected: false},
            Test{input: "(1 > 2) == true", expected: false},
            Test{input: "(1 > 2) == false", expected: true},
        ];

        for t in tests {
            let evaluated = test_eval(t.input);

            test_boolean_object(&evaluated, t.expected);
        }
    }

    #[test]
    fn bang_operator() {
        struct Test<'a> {
            input: &'a str,
            expected: bool,
        }
        let tests = vec![
            Test{input: "!true", expected: false},
            Test{input: "!false", expected: true},
            Test{input: "!5", expected: false},
            Test{input: "!!true", expected: true},
            Test{input: "!!false", expected: false},
            Test{input: "!!5", expected: true},
        ];

        for t in tests {
            let evaluated = test_eval(t.input);
            test_boolean_object(&evaluated, t.expected);
        }
    }

    #[test]
    fn if_else_expressions() {
        struct Test<'a> {
            input: &'a str,
            expected: Object,
        }
        let tests = vec![
            Test{input: "if (true) { 10 }", expected: Object::Int(10)},
            Test{input: "if (false) { 10 }", expected: Object::Null},
            Test{input: "if (1) { 10 }", expected: Object::Int(10)},
            Test{input: "if (1 < 2) { 10 }", expected: Object::Int(10)},
            Test{input: "if (1 > 2) { 10 }", expected: Object::Null},
            Test{input: "if (1 > 2) { 10 } else { 20 }", expected: Object::Int(20)},
            Test{input: "if (1 < 2) { 10 } else { 20 }", expected: Object::Int(10)},
        ];

        for t in tests {
            let evaluated = &*test_eval(t.input);

            match t.expected {
                Object::Int(i) => test_integer_object(&evaluated, i),
                _ => test_null_object(&evaluated),
            }
        }
    }

    #[test]
    fn return_statements() {
        struct Test<'a> {
            input: &'a str,
            expected: i64,
        }
        let tests = vec![
            Test{input: "return 10;", expected: 10},
            Test{input: "return 10; 9;", expected: 10},
            Test{input: "return 2 * 5; 9;", expected: 10},
            Test{input: "9; return 2 * 5; 9;", expected: 10},
            Test{input: "if (10 > 1) {
                           if (10 > 1) {
                             return 10;
                           }
                           return 1;
                         }", expected: 10},
        ];

        for t in tests {
            let evaluated = test_eval(t.input);
            test_integer_object(&evaluated, t.expected)
        }
    }

    #[test]
    fn error_handling() {
        struct Test<'a> {
            input: &'a str,
            expected: &'a str,
        }
        let tests = vec![
            Test{input: "5 + true;", expected: "type mismatch: Int(5) + Bool(true)"},
            Test{input: "5 + true; 5;", expected: "type mismatch: Int(5) + Bool(true)"},
            Test{input: "-true", expected: "unknown operator: -Bool(true)"},
            Test{input: "true + false", expected: "unknown operator: true + false"},
            Test{input: "5; true + false; 5", expected: "unknown operator: true + false"},
            Test{input: "if (10 > 1) { true + false; }", expected: "unknown operator: true + false"},
            Test{input: "if (10 > 1) {
                             if (10 > 1) {
                                return true + false;
                             }

                             return 1;
                          }", expected: "unknown operator: true + false"},
            Test{input: "foobar", expected: "identifier not found: foobar"},
            Test{input: r#" {"name": "Monkey"}[fn(x) { x }]; "#, expected: "unusable as hash key: fn(x) {\nx\n}"},
        ];

        for t in tests {
            let env = Rc::new(RefCell::new(Environment::new()));
            match parser::parse(t.input) {
                Ok(node) => {
                    match eval(&node, env) {
                        Err(e) => assert_eq!(e.message, t.expected),
                        n => panic!("expected error {} but got {:?}", t.expected, n)
                    }
                },
                Err(e) => panic!("error {:?} on input {}", e, t.input),
            }
        }
    }

    #[test]
    fn let_statements() {
        struct Test<'a> {
            input: &'a str,
            expected: i64,
        }
        let tests = vec![
            Test{input: "let a = 5; a;", expected: 5},
            Test{input: "let a = 5 * 5; a;", expected: 25},
            Test{input: "let a = 5; let b = a; b;", expected: 5},
            Test{input: "let a = 5; let b = a; let c = a + b + 5; c;", expected: 15},
        ];

        for t in tests {
            let evaluated = test_eval(t.input);
            test_integer_object(&evaluated, t.expected)
        }
    }

    #[test]
    fn function_object() {
        let input = "fn(x) { x + 2; };";
        let evaluated = &*test_eval(input);

        match evaluated {
            Object::Function(f) => {
                assert_eq!(f.parameters.len(), 1);
                assert_eq!(f.parameters.first().unwrap().name, "x");
                assert_eq!(f.body.to_string(), "(x + 2)");
            },
            _ => panic!("expected function object but got {:?}", evaluated)
        }
    }
    #[test]
    fn function_application() {
        struct Test<'a> {
            input: &'a str,
            expected: i64,
        }
        let tests = vec![
            Test{input: "let identity = fn(x) { x; }; identity(5);", expected: 5},
            Test{input: "let identity = fn(x) { return x; }; identity(5);", expected: 5},
            Test{input: "let double = fn(x) { x * 2; }; double(5);", expected: 10},
            Test{input: "let add = fn(x, y) { x + y; }; add(5, 5);", expected: 10},
            Test{input: "let add = fn(x, y) { x + y; }; add(5 + 5, add(5, 5));", expected: 20},
            Test{input: "fn(x) { x; }(5)", expected: 5},
        ];

        for t in tests {
            test_integer_object(&test_eval(t.input), t.expected)
        }
    }

    #[test]
    fn closures() {
        let input = "let newAdder = fn(x) {
  fn(y) { x + y };
};

let addTwo = newAdder(2);
addTwo(2);";
        test_integer_object(&test_eval(input), 4)
    }

    #[test]
    fn string_literal() {
        let input = r#""Hello World!"#;

        match &*test_eval(input) {
            Object::String(s) => assert_eq!(s, "Hello World!"),
            obj => panic!(format!("expected string but got {:?}", obj))
        }

    }

    #[test]
    fn string_concatenation() {
        let input = r#""Hello" + " " + "World!""#;

        match &*test_eval(input) {
            Object::String(s) => assert_eq!(s, "Hello World!"),
            obj => panic!(format!("expected string but got {:?}", obj))
        }
    }

    #[test]
    fn builtin_functions() {
        struct Test<'a> {
            input: &'a str,
            expected: Object,
        }
        let tests = vec![
            Test{input: r#"len("")"#, expected: Object::Int(0)},
            Test{input: r#"len("four")"#, expected: Object::Int(4)},
            Test{input: r#"len("hello world")"#, expected: Object::Int(11)},
            Test{input: "len([1, 2, 3])", expected: Object::Int(3)},
            Test{input: "len([])", expected: Object::Int(0)},
            Test{input: "first([1, 2, 3])", expected: Object::Int(1)},
            Test{input: "first([])", expected: Object::Null},
            Test{input: "last([1, 2, 3])", expected: Object::Int(3)},
            Test{input: "last([])", expected: Object::Null},
            Test{input: "rest([1, 2, 3])", expected: Object::Array(Rc::new(Array{elements: vec![Rc::new(Object::Int(2)), Rc::new(Object::Int(3))]}))},
            Test{input: "rest([])", expected: Object::Array(Rc::new(Array{elements: vec![]}))},
            Test{input: "push([], 1)", expected: Object::Array(Rc::new(Array{elements: vec![Rc::new(Object::Int(1))]}))},
        ];

        for t in tests {
            let obj = test_eval(t.input);

            match (&t.expected, &*obj) {
                (Object::Int(exp), Object::Int(got)) => assert_eq!(*exp, *got, "on input {} expected {} but got {}", t.input, exp, got),
                (Object::Null, Object::Null) => {},
                (Object::Array(ex), Object::Array(got)) => {
                    assert_eq!(ex.elements.len(), got.elements.len());
                    let mut got_iter = (&got.elements).into_iter();
                    for obj in &ex.elements {
                        let got_obj = Rc::clone(got_iter.next().unwrap());
                        match (&*Rc::clone(obj), &*got_obj) {
                            (Object::Int(exi), Object::Int(goti)) => assert_eq!(*exi, *goti),
                            _ => panic!("{:?} not same type as {:?}", got_obj, obj)
                        }
                    }
                }
                _ => panic!("on input {} expected {:?} but got {:?}", t.input, t.expected, obj)
            }
        }
    }

    #[test]
    fn builtin_errors() {
        struct Test<'a> {
            input: &'a str,
            expected: &'a str,
        }
        let tests = vec![
            Test{input: r#"len(1)"#, expected: "object Int(1) not supported as an argument for len"},
            Test{input: r#"len("one", "two")"#, expected: "len takes only 1 array or string argument"},
            Test{input: r#"first(1)"#, expected: "object Int(1) not supported as an argument for first"},
        ];

        for t in tests {
            let env = Rc::new(RefCell::new(Environment::new()));
            match parser::parse(t.input) {
                Ok(node) => {
                    match eval(&node, env) {
                        Ok(obj) => panic!("expected error on input {} but got {:?}", t.input, obj),
                        Err(err) => assert_eq!(t.expected, err.message, "on input {} expected error {} but got {}", t.input, t.expected, err.message)
                    }
                },
                Err(e) => panic!("error {:?} on input {}", e, t.input),
            }
        }
    }

    #[test]
    fn array_literals() {
        let input = "[1, 2 * 2, 3 + 3]";
        let obj = test_eval(input);
        match  &*obj {
            Object::Array(a) => {
                test_integer_object(a.elements.get(0).unwrap(), 1);
                test_integer_object(a.elements.get(1).unwrap(), 4);
                test_integer_object(a.elements.get(2).unwrap(), 6);
            },
            _ => panic!("expected array but got {:?}", obj)
        }
    }

    #[test]
    fn array_index_expressions() {
        struct Test<'a> {
            input: &'a str,
            expected: i64,
        }
        let tests = vec![
            Test{input: "[1, 2, 3][0]", expected: 1},
            Test{input: "[1, 2, 3][1]", expected: 2},
            Test{input: "[1, 2, 3][2]", expected: 3},
            Test{input: "let i = 0; [1][i];", expected: 1},
            Test{input: "[1, 2, 3][1 + 1];", expected: 3},
            Test{input: "let myArray = [1, 2, 3]; myArray[2];", expected: 3},
            Test{input: "let myArray = [1, 2, 3]; myArray[0] + myArray[1] + myArray[2];", expected: 6},
            Test{input: "let myArray = [1, 2, 3]; let i = myArray[0]; myArray[i]", expected: 2},
        ];

        for t in tests {
            let obj = test_eval(t.input);
            match &*obj {
                Object::Int(i) => assert_eq!(*i, t.expected),
                _ => panic!("expected int obj but got {:?}", obj)
            }
        }
    }

    #[test]
    fn invalid_array_index() {
        let inputs = vec![
            "[1, 2, 3][3]",
            "[1, 2, 3][-1]",
        ];

        for input in inputs {
            let obj = test_eval(input);
            match &*obj {
                Object::Null => {},
                _ => panic!("expected null object, but got {:?}", obj)
            }
        }
    }

    #[test]
    fn hash_literal() {
        let input = r#"let two = "two";
            {
                "one": 10 - 9,
                two: 1 + 1,
                "thr" + "ee": 6 / 2,
                4: 4,
                true: 5,
                false: 6
            }
        "#;

        let obj = test_eval(input);
        match &*obj {
            Object::Hash(h) => {
                assert_eq!(h.pairs.len(), 6);

                for (key, value) in &h.pairs {
                    match (&*Rc::clone(key), &*Rc::clone(value)) {
                        (Object::String(k), Object::Int(val)) => {
                            match k.as_str() {
                                "one" => assert_eq!(*val, 1),
                                "two" => assert_eq!(*val, 2),
                                "three" => assert_eq!(*val, 3),
                                _ => panic!("unexpected string key {}", k)
                            }
                        },
                        (Object::Bool(b), Object::Int(val)) => {
                            if *b {
                                assert_eq!(*val, 5)
                            } else {
                                assert_eq!(*val, 6)
                            }
                        },
                        (Object::Int(k), Object::Int(val)) => assert_eq!(k, val),
                        _ => panic!("unexpected key value pair {:?} {:?}", key, value)
                    }
                }
            },
            _ => panic!("expected hash object, but got {:?}", obj)
        }
    }

    #[test]
    fn hash_index_expressions() {
        struct Test<'a> {
            input: &'a str,
            expected: Object,
        }
        let tests = vec![
            Test{input: r#" {"foo":5}["foo"] "#, expected: Object::Int(5)},
            Test{input: r#" {"foo":5}["bar"] "#, expected: Object::Null},
            Test{input: r#" let key = "foo"; {"foo":5}[key] "#, expected: Object::Int(5)},
            Test{input: r#" {}["foo"] "#, expected: Object::Null},
            Test{input: r#" {5: 5}[5] "#, expected: Object::Int(5)},
            Test{input: r#" {true: 5}[true] "#, expected: Object::Int(5)},
            Test{input: r#" {false: 5}[false] "#, expected: Object::Int(5)},
        ];

        for t in tests {
            let obj = test_eval(t.input);

            match (&t.expected, &*obj) {
                (Object::Int(exp), Object::Int(got)) => assert_eq!(*exp, *got, "on input {} expected {} but got {}", t.input, exp, got),
                (Object::Null, Object::Null) => {},
                _ => panic!("on input {} expected {:?} but got {:?}", t.input, t.expected, obj)
            }
        }
    }

    fn test_eval(input: &str) -> Rc<Object> {
        let env = Rc::new(RefCell::new(Environment::new()));
        match parser::parse(input) {
            Ok(node) => {
                eval(&node, env).expect(input)

            },
            Err(e) => panic!("error {:?} on input {}", e, input),
        }
    }

    fn test_integer_object(obj: &Object, expected: i64) {
        match obj {
            Object::Int(i) => assert_eq!(i, &expected),
            _ => panic!("expected integer object, but got {:?}", obj),
        }
    }

    fn test_boolean_object(obj: &Object, expected: bool) {
        match obj {
            Object::Bool(b) => assert_eq!(b, &expected),
            _ => panic!("expected boolean object, but got {:?}", obj),
        }
    }

    fn test_null_object(obj: &Object) {
        match obj {
            Object::Null => {},
            _ => panic!("expected null but got {:?}", obj),
        }
    }
}