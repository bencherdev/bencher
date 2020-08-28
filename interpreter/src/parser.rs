use crate::ast::*;
use std::collections::HashMap;
use crate::token::Token;
use crate::lexer::Lexer;

type ParseError = String;
type ParseErrors = Vec<ParseError>;
pub type ParseResult<T> = Result<T, ParseError>;
type PrefixFn = fn(parser: &mut Parser<'_>) -> ParseResult<Expression>;
type InfixFn = fn(parser: &mut Parser<'_>, left: Expression) -> ParseResult<Expression>;

#[derive(PartialEq, PartialOrd)]
enum Precedence {
    Lowest,
    Equals,
    LessGreater,
    Sum,
    Product,
    Prefix,
    Call,
    Index,
}

impl Precedence {
    fn token_precedence(tok: &Token) -> Precedence {
        match tok {
            Token::Eq => Precedence::Equals,
            Token::Neq => Precedence::Equals,
            Token::Lt => Precedence::LessGreater,
            Token::Gt => Precedence::LessGreater,
            Token::Plus => Precedence::Sum,
            Token::Minus => Precedence::Sum,
            Token::Slash => Precedence::Product,
            Token::Asterisk => Precedence::Product,
            Token::Lparen => Precedence::Call,
            Token::Lbracket => Precedence::Index,
            _ => Precedence::Lowest,
        }
    }
}

pub struct Parser<'a> {
    l: Lexer<'a>,

    cur_token: Token,
    peek_token: Token,
}

pub fn parse(input: &str) -> Result<Node, ParseErrors> {
    let l = Lexer::new(input);
    let mut p = Parser::new(l);
    let prog = p.parse_program()?;

    Ok(Node::Program(Box::new(prog)))
}

impl<'a> Parser<'a> {
    pub fn new(l: Lexer<'_>) -> Parser<'_> {
        let mut l = l;
        let cur = l.next_token();
        let next = l.next_token();
        Parser {
            l,
            cur_token: cur,
            peek_token: next,
        }
    }

    pub fn parse_program(&mut self) -> Result<Program, ParseErrors> {
        let mut prog = Program::new();
        let mut errors = ParseErrors::new();
        let mut tok = self.cur_token.clone();

        while tok != Token::EOF {
            match self.parse_statement() {
                Ok(stmt) => prog.statements.push(stmt),
                Err(err) => errors.push(err),
            }
            self.next_token();
            tok = self.cur_token.clone();
        }

        if errors.len() > 0 {
            return Err(errors);
        }

        Ok(prog)
    }

    fn parse_statement(&mut self) -> ParseResult<Statement> {
        match &self.cur_token.clone() {
            Token::Let => self.parse_let_statement(),
            Token::Return => self.parse_return_statement(),
            _ => self.parse_expression_statement(),
        }
    }

    fn parse_expression_statement(&mut self) -> ParseResult<Statement> {
        let expression = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token_is(&Token::Semicolon) {
            self.next_token();
        }

        Ok(Statement::Expression(Box::new(ExpressionStatement { expression })))
    }

    fn parse_expression(&mut self, precedence: Precedence) -> ParseResult<Expression> {
        let mut left_exp;

        if let Some(f) = self.prefix_fn() {
            left_exp = f(self)?;
        } else {
            return Err(format!("no prefix parse function for {} found", self.cur_token));
        }

        while !self.peek_token_is(&Token::Semicolon) && precedence < self.peek_precedence() {
            match self.infix_fn() {
                Some(f) => {
                    self.next_token();
                    left_exp = f(self, left_exp)?;
                }
                None => return Ok(left_exp),
            }
        }

        Ok(left_exp)
    }

    fn prefix_fn(&mut self) -> Option<PrefixFn> {
        match self.cur_token {
            Token::Ident(_) => Some(Parser::parse_identifier),
            Token::Int(_) => Some(Parser::parse_integer_literal),
            Token::String(_) => Some(Parser::parse_string_literal),
            Token::Bang | Token::Minus => Some(Parser::parse_prefix_expression),
            Token::True | Token::False => Some(Parser::parse_boolean),
            Token::Lparen => Some(Parser::parse_grouped_expression),
            Token::If => Some(Parser::parse_if_expression),
            Token::Function => Some(Parser::parse_function_literal),
            Token::Lbracket => Some(Parser::parse_array_literal),
            Token::Lbrace => Some(Parser::parse_hash_literal),
            _ => None,
        }
    }

    fn parse_hash_literal(parser: &mut Parser<'_>) -> ParseResult<Expression> {
        let mut pairs: HashMap<Expression,Expression> = HashMap::new();

        while !parser.peek_token_is(&Token::Rbrace) {
            parser.next_token();
            let key = parser.parse_expression(Precedence::Lowest)?;

            parser.expect_peek(Token::Colon)?;
            parser.next_token();
            let value = parser.parse_expression(Precedence::Lowest)?;

            pairs.insert(key, value);

            if !parser.peek_token_is(&Token::Rbrace) {
                parser.expect_peek(Token::Comma)?;
            }
        }

        parser.expect_peek(Token::Rbrace)?;

        Ok(Expression::Hash(Box::new(HashLiteral{pairs})))
    }

    fn parse_array_literal(parser: &mut Parser<'_>) -> ParseResult<Expression> {
        let elements = parser.parse_expression_list(Token::Rbracket)?;
        Ok(Expression::Array(Box::new(ArrayLiteral{elements})))
    }

    fn parse_expression_list(&mut self, end: Token) -> ParseResult<Vec<Expression>> {
        let mut list: Vec<Expression> = Vec::new();

        if self.peek_token_is(&end) {
            self.next_token();
            return Ok(list)
        }

        self.next_token();
        list.push(self.parse_expression(Precedence::Lowest)?);

        while self.peek_token_is(&Token::Comma) {
            self.next_token();
            self.next_token();
            list.push(self.parse_expression(Precedence::Lowest)?);
        }

        self.expect_peek(end)?;

        Ok(list)
    }

    fn parse_prefix_expression(parser: &mut Parser<'_>) -> ParseResult<Expression> {
        let operator = parser.cur_token.clone();

        parser.next_token();

        let right = parser.parse_expression(Precedence::Prefix)?;

        Ok(Expression::Prefix(Box::new(PrefixExpression { operator, right })))
    }

    fn parse_if_expression(parser: &mut Parser<'_>) -> ParseResult<Expression> {
        parser.expect_peek(Token::Lparen)?;
        parser.next_token();

        let condition = parser.parse_expression(Precedence::Lowest)?;

        parser.expect_peek(Token::Rparen)?;
        parser.expect_peek(Token::Lbrace)?;

        let consequence = parser.parse_block_statement()?;

        let alternative = if parser.peek_token_is(&Token::Else) {
            parser.next_token();

            parser.expect_peek(Token::Lbrace)?;

            let alt_block = parser.parse_block_statement()?;
            Some(alt_block)
        } else {
            None
        };

        Ok(Expression::If(Box::new(IfExpression { condition, consequence, alternative })))
    }

    fn parse_block_statement(&mut self) -> ParseResult<BlockStatement> {
        let mut statements = Vec::new();

        self.next_token();

        while !self.cur_token_is(Token::Rbrace) && !self.cur_token_is(Token::EOF) {
            if let Ok(stmt) = self.parse_statement() {
                statements.push(stmt);
            }
            self.next_token();
        }

        Ok(BlockStatement { statements })
    }

    fn parse_function_literal(parser: &mut Parser<'_>) -> ParseResult<Expression> {
        parser.expect_peek(Token::Lparen)?;

        let parameters = parser.parse_function_parameters()?;

        parser.expect_peek(Token::Lbrace)?;

        let body = parser.parse_block_statement()?;

        Ok(Expression::Function(Box::new(FunctionLiteral{parameters,body})))
    }

    fn parse_function_parameters(&mut self) -> Result<Vec<IdentifierExpression>, ParseError> {
        let mut identifiers: Vec<IdentifierExpression> = Vec::new();

        if self.peek_token_is(&Token::Rparen) {
            self.next_token();
            return Ok(identifiers)
        }

        self.next_token();

        identifiers.push(self.parse_identifier_into_identifier_expression()?);

        while self.peek_token_is(&Token::Comma) {
            self.next_token();
            self.next_token();
            identifiers.push(self.parse_identifier_into_identifier_expression()?);
        }

        self.expect_peek(Token::Rparen)?;

        Ok(identifiers)
    }

    fn parse_grouped_expression(parser: &mut Parser<'_>) -> ParseResult<Expression> {
        parser.next_token();

        let exp = parser.parse_expression(Precedence::Lowest);

        parser.expect_peek(Token::Rparen)?;

        exp
    }

    fn infix_fn(&mut self) -> Option<InfixFn> {
        match self.peek_token {
            Token::Plus | Token::Minus | Token::Slash | Token::Asterisk | Token::Eq | Token::Neq | Token::Lt | Token::Gt => Some(Parser::parse_infix_expression),
            Token::Lparen => Some(Parser::parse_call_expression),
            Token::Lbracket => Some(Parser::parse_index_expression),
            _ => None,
        }
    }

    fn parse_index_expression(parser: &mut Parser<'_>, left: Expression) -> ParseResult<Expression> {
        parser.next_token();

        let exp = IndexExpression{left, index: parser.parse_expression(Precedence::Lowest)?};

        parser.expect_peek(Token::Rbracket)?;

        Ok(Expression::Index(Box::new(exp)))
    }

    fn parse_call_expression(parser: &mut Parser<'_>, function: Expression) -> ParseResult<Expression> {
        let arguments = parser.parse_expression_list(Token::Rparen)?;
        Ok(Expression::Call(Box::new(CallExpression{function, arguments})))
    }

    fn parse_infix_expression(parser: &mut Parser<'_>, left: Expression) -> ParseResult<Expression> {
        let operator = parser.cur_token.clone();
        let precedence = parser.cur_precedence();

        parser.next_token();

        let right = parser.parse_expression(precedence)?;

        Ok(Expression::Infix(Box::new(InfixExpression { operator, left, right })))
    }

    fn parse_boolean(parser: &mut Parser<'_>) -> ParseResult<Expression> {
        match parser.cur_token {
            Token::True => Ok(Expression::Boolean(true)),
            Token::False => Ok(Expression::Boolean(false)),
            // we should never hit this since this function is only handed out for tokens matched as boolean
            _ => panic!("couldn't parse {:?} to boolean", parser.cur_token)
        }
    }

    fn parse_identifier_into_identifier_expression(&mut self) -> ParseResult<IdentifierExpression> {
        if let Token::Ident(ref name) = self.cur_token {
            return Ok(IdentifierExpression { name: name.to_string() });
        }

        Err(format!("unexpected error on identifier parse with {}", self.cur_token))
    }

    fn parse_identifier(parser: &mut Parser<'_>) -> ParseResult<Expression> {
        if let Token::Ident(ref name) = parser.cur_token {
            return Ok(Expression::Identifier(name.to_string()));
        }

        Err(format!("unexpected error on identifier parse with {}", parser.cur_token))
    }

    fn parse_string_literal(parser: &mut Parser<'_>) -> ParseResult<Expression> {
        if let Token::String(ref s) = parser.cur_token {
            return Ok(Expression::String(s.to_string()));
        }

        Err(format!("unexpected error on string parse with {}", parser.cur_token))
    }

    fn parse_integer_literal(parser: &mut Parser<'_>) -> ParseResult<Expression> {
        if let Token::Int(value) = parser.cur_token {
            return Ok(Expression::Integer(value));
        }

        Err(format!("error parsing integer literal {}", parser.cur_token))
    }

    fn parse_return_statement(&mut self) -> ParseResult<Statement> {
        self.next_token();

        let value = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token_is(&Token::Semicolon) {
            self.next_token();
        }

        Ok(Statement::Return(Box::new(ReturnStatement { value })))
    }

    fn parse_let_statement(&mut self) -> ParseResult<Statement> {
        let name = self.expect_ident()?;

        self.expect_peek(Token::Assign)?;

        self.next_token();

        let value = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token_is(&Token::Semicolon) {
            self.next_token();
        }

        Ok(Statement::Let(Box::new(LetStatement { name, value })))
    }

    fn peek_precedence(&self) -> Precedence {
        Precedence::token_precedence(&self.peek_token)
    }

    fn cur_precedence(&self) -> Precedence {
        Precedence::token_precedence(&self.cur_token)
    }

    fn cur_token_is(&self, tok: Token) -> bool {
        match (&tok, &self.cur_token) {
            (Token::Ident(_), Token::Ident(_)) => true,
            (Token::Int(_), Token::Int(_)) => true,
            _ => tok == self.cur_token,
        }
    }

    fn peek_token_is(&self, tok: &Token) -> bool {
        match (&tok, &self.peek_token) {
            (Token::Ident(_), Token::Ident(_)) => true,
            (Token::Int(_), Token::Int(_)) => true,
            _ => tok == &self.peek_token,
        }
    }

    fn expect_peek(&mut self, tok: Token) -> Result<(), ParseError> {
        match self.peek_token_is(&tok) {
            true => {
                self.next_token();
                Ok(())
            }
            false => Err(format!("expected next token to be {} got {} instead", tok, self.peek_token))
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        let name = match &self.peek_token {
            Token::Ident(name) => name.to_string(),
            _ => return Err(format!("invalid identifier {}", self.peek_token)),
        };

        self.next_token();
        Ok(name)
    }

    fn next_token(&mut self) {
        self.cur_token = self.peek_token.clone();
        self.peek_token = self.l.next_token();
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::lexer::Lexer;

    #[test]
    fn let_statement() {
        let input = "\
let x = 5;
let y = 10;
let foobar = 838383;";

        let prog = setup(input, 3);

        let tests = vec![
            "x",
            "y",
            "foobar",
        ];

        let mut itr = prog.statements.iter();

        for t in tests {
            match itr.next().unwrap() {
                Statement::Let(ref l) => {
                    assert_eq!(l.name, t);

                },
                _ => panic!("unknown node")
            }
        }
    }

    #[test]
    fn let_statement_bool() {
        let prog = setup("let y = true;", 1);
        let exp = let_statement_parse_and_verify(&prog, "y");
        match exp {
            Expression::Boolean(b) => assert_eq!(b, &true),
            _ => panic!("expected boolean expression")
        }
    }

    #[test]
    fn let_statement_ident() {
        let prog = setup("let foobar = y;", 1);
        let exp = let_statement_parse_and_verify(&prog, "foobar");
        match exp {
            Expression::Identifier(_) => test_identifier(&exp, "y"),
            _ => panic!("expected identifier expression")
        }
    }

    fn let_statement_parse_and_verify<'a>(prog: &'a Program, expected_ident: &str) -> &'a Expression {
        let stmt = prog.statements.first().unwrap();
        match stmt {
            Statement::Let(stmt) => {
                assert_eq!(stmt.name.as_str(), expected_ident);
                return &stmt.value
            },
            stmt => panic!("expected let statement but got {:?}", stmt)
        }
    }

    #[test]
    fn let_statement_error() {
        let input = "\
let x 5;
let = 10;
let y = 23;
let 23432";

        let l = Lexer::new(input);
        let mut p = Parser::new(l);

        match p.parse_program() {
            Ok(_) => panic!("should have retured a parse failure"),
            Err(errors) => {
                if errors.len() != 4 {
                    panic!("got {} errors instead of 3\n{:?}", errors.len(), errors)
                }

                let expected_errors = vec![
                    "expected next token to be = got 5 instead",
                    "invalid identifier =",
                    "no prefix parse function for = found",
                    "invalid identifier 23432"
                ];

                let mut itr = errors.iter();

                for err in expected_errors {
                    let message = itr.next().unwrap();
                    assert_eq!(message, err, "expected error '{}' but got '{}'", err, message)
                }
            }
        }
    }

    #[test]
    fn return_statement() {
        let input = "\
return 5;\
return 10;\
return 993322;";

        let prog = setup(input, 3);

        for stmt in prog.statements {
            match stmt {
                Statement::Return(_) => {}
                _ => panic!("statement {:?} isn't a return statement", stmt),
            }
        }
    }

    #[test]
    fn return_statement_bool() {
        let prog = setup("return true;", 1);
        let exp = return_statement_parse_and_verify(&prog);
        match exp {
            Expression::Boolean(b) => assert_eq!(b, &true),
            _ => panic!("expected boolean expression")
        }
    }

    #[test]
    fn return_statement_ident() {
        let prog = setup("return foobar;", 1);
        let exp = return_statement_parse_and_verify(&prog);
        match exp {
            Expression::Identifier(_) => test_identifier(&exp, "foobar"),
            _ => panic!("expected identifier expression")
        }
    }

    fn return_statement_parse_and_verify<'a>(prog: &'a Program) -> &'a Expression {
        let stmt = prog.statements.first().unwrap();
        match stmt {
            Statement::Return(stmt) => {
                return &stmt.value
            },
            stmt => panic!("expected return statement but got {:?}", stmt)
        }
    }

    #[test]
    fn identifier_expression() {
        let input = "foobar;";

        let prog = setup(input, 1);
        let exp = unwrap_expression(&prog);

        test_identifier(exp, "foobar");
    }

    #[test]
    fn integer_literal() {
        let input = "5;";

        let prog = setup(input, 1);
        let exp = unwrap_expression(&prog);

        match exp {
            Expression::Integer(int) => assert_eq!(*int, 5, "expected value to be 5 but got {}", int),
            exp => panic!("expected integer literal expression but got {:?}", exp)
        }
    }

    #[test]
    fn prefix_expressions() {
        struct Test<'a> {
            input: &'a str,
            operator: Token,
            value: i64,
        }

        // TODO: add tests for boolean prefix expressions like !true; and !false;
        let tests = vec![
            Test { input: "!5;", operator: Token::Bang, value: 5 },
            Test { input: "-15;", operator: Token::Minus, value: 15 },
        ];

        for t in tests {
            let prog = setup(t.input, 1);
            let exp = unwrap_expression(&prog);

            match exp {
                Expression::Prefix(prefix) => {
                    assert_eq!(t.operator, prefix.operator, "expected {} operator but got {}", t.operator, prefix.operator);
                    test_integer_literal(&prefix.right, t.value);
                }
                exp => panic!("expected prefix expression but got {:?}", exp)
            }
        }
    }

    #[test]
    fn infix_expressions() {
        struct Test<'a> {
            input: &'a str,
            left_value: i64,
            operator: Token,
            right_value: i64,
        }

        let tests = vec![
            Test { input: "5 + 5;", left_value: 5, operator: Token::Plus, right_value: 5 },
            Test { input: "5 - 5;", left_value: 5, operator: Token::Minus, right_value: 5 },
            Test { input: "5 * 5;", left_value: 5, operator: Token::Asterisk, right_value: 5 },
            Test { input: "5 / 5;", left_value: 5, operator: Token::Slash, right_value: 5 },
            Test { input: "5 > 5;", left_value: 5, operator: Token::Gt, right_value: 5 },
            Test { input: "5 < 5;", left_value: 5, operator: Token::Lt, right_value: 5 },
            Test { input: "5 == 5;", left_value: 5, operator: Token::Eq, right_value: 5 },
            Test { input: "5 != 5;", left_value: 5, operator: Token::Neq, right_value: 5 },
        ];

        for t in tests {
            let prog = setup(t.input, 1);
            let exp = unwrap_expression(&prog);

            match exp {
                Expression::Infix(infix) => {
                    assert_eq!(t.operator, infix.operator, "expected {} operator but got {}", t.operator, infix.operator);
                    test_integer_literal(&infix.left, t.left_value);
                    test_integer_literal(&infix.right, t.right_value);
                }
                exp => panic!("expected prefix expression but got {:?}", exp)
            }
        }
    }

    #[test]
    fn infix_boolean_literal_expressions() {
        struct Test<'a> {
            input: &'a str,
            left_value: bool,
            operator: Token,
            right_value: bool,
        }

        let tests = vec![
            Test { input: "true == true", left_value: true, operator: Token::Eq, right_value: true },
            Test { input: "true != false", left_value: true, operator: Token::Neq, right_value: false },
            Test { input: "false == false", left_value: false, operator: Token::Eq, right_value: false },
        ];

        for t in tests {
            let prog = setup(t.input, 1);
            let exp = unwrap_expression(&prog);

            match exp {
                Expression::Infix(infix) => {
                    assert_eq!(t.operator, infix.operator, "expected {} operator but got {}", t.operator, infix.operator);
                    test_boolean_literal(&infix.left, t.left_value);
                    test_boolean_literal(&infix.right, t.right_value);
                }
                exp => panic!("expected infix expression but got {:?}", exp)
            }
        }
    }

    #[test]
    fn operator_precedence() {
        struct Test<'a> {
            input: &'a str,
            expected: &'a str,
        }

        let tests = vec![
            Test { input: "-a * b", expected: "((-a) * b)" },
            Test { input: "!-a", expected: "(!(-a))" },
            Test { input: "a + b + c", expected: "((a + b) + c)" },
            Test { input: "a + b - c", expected: "((a + b) - c)" },
            Test { input: "a * b * c", expected: "((a * b) * c)" },
            Test { input: "a * b / c", expected: "((a * b) / c)" },
            Test { input: "a + b / c", expected: "(a + (b / c))" },
            Test { input: "a + b * c + d / e - f", expected: "(((a + (b * c)) + (d / e)) - f)" },
            Test { input: "3 + 4; -5 * 5", expected: "(3 + 4)((-5) * 5)" },
            Test { input: "5 > 4 == 3 < 4", expected: "((5 > 4) == (3 < 4))" },
            Test { input: "5 < 4 != 3 > 4", expected: "((5 < 4) != (3 > 4))" },
            Test { input: "3 + 4 * 5 == 3 * 1 + 4 * 5", expected: "((3 + (4 * 5)) == ((3 * 1) + (4 * 5)))" },
            Test { input: "true", expected: "true" },
            Test { input: "false", expected: "false" },
            Test { input: "3 > 5 == false", expected: "((3 > 5) == false)" },
            Test { input: "3 < 5 == true", expected: "((3 < 5) == true)" },
            Test { input: "1 + (2 + 3) + 4", expected: "((1 + (2 + 3)) + 4)" },
            Test { input: "(5 + 5) * 2", expected: "((5 + 5) * 2)" },
            Test { input: "2 / (5 + 5)", expected: "(2 / (5 + 5))" },
            Test { input: "-(5 + 5)", expected: "(-(5 + 5))" },
            Test { input: "!(true == true)", expected: "(!(true == true))" },
            Test { input: "a + add(b * c) + d", expected: "((a + add((b * c))) + d)" },
            Test { input: "add(a, b, 1, 2 * 3, 4 + 5, add(6, 7 * 8))", expected: "add(a, b, 1, (2 * 3), (4 + 5), add(6, (7 * 8)))" },
            Test { input: "add(a + b + c * d / f + g)", expected: "add((((a + b) + ((c * d) / f)) + g))" },
            Test { input: "a * [1, 2, 3, 4][b * c] * d", expected: "((a * ([1, 2, 3, 4][(b * c)])) * d)" },
            Test { input: "add(a * b[2], b[1], 2 * [1, 2][1])", expected: "add((a * (b[2])), (b[1]), (2 * ([1, 2][1])))" },
        ];

        for t in tests {
            let prog = setup(t.input, 0).to_string();

            assert_eq!(t.expected, prog, "expected '{}' but got '{}'", t.expected, prog)
        }
    }

    #[test]
    fn boolean_expression() {
        struct Test<'a> {
            input: &'a str,
            expected: bool,
        }

        let tests = vec![
            Test { input: "true;", expected: true },
            Test { input: "false;", expected: false },
        ];

        for t in tests {
            let prog = setup(t.input, 1);
            let exp = unwrap_expression(&prog);

            test_boolean_literal(&exp, t.expected);
        }
    }

    #[test]
    fn if_expression() {
        let input = "if (x < y) { x }";

        let prog = setup(input, 1);
        let exp = unwrap_expression(&prog);

        match exp {
            Expression::If(ifexpr) => {
                test_if_condition(&ifexpr.condition, Token::Lt, "x", "y");

                assert_eq!(ifexpr.consequence.statements.len(), 1, "expected only 1 statement");
                match ifexpr.consequence.statements.first().unwrap() {
                    Statement::Expression(stmt) => test_identifier(&stmt.expression, "x"),
                    stmt => panic!("expected expression statement but got {:?}", stmt)
                }
                if let Some(stmt) = &ifexpr.alternative {
                    panic!("expected alternative to be None but got {:?}", stmt)
                }
            }
            _ => panic!("expected if expression but got {:?}", exp)
        }
    }

    #[test]
    fn if_else_expression() {
        let input = "if (x < y) { x } else { y }";

        let prog = setup(input, 1);
        let exp = unwrap_expression(&prog);

        match exp {
            Expression::If(ifexpr) => {
                test_if_condition(&ifexpr.condition, Token::Lt, "x", "y");

                assert_eq!(ifexpr.consequence.statements.len(), 1);
                match &ifexpr.consequence.statements.first().unwrap() {
                    Statement::Expression(stmt) => test_identifier(&stmt.expression, "x"),
                    stmt => panic!("expected expression statement but got {:?}", stmt)
                }

                if let Some(stmt) = &ifexpr.alternative {
                    assert_eq!(stmt.statements.len(), 1);
                    match stmt.statements.first().unwrap() {
                        Statement::Expression(stmt) => test_identifier(&stmt.expression, "y"),
                        stmt => panic!("expected expression statement but got {:?}", stmt)
                    }
                } else {
                    panic!("expected alternative block")
                }
            }
            _ => panic!("expected if expression but got {:?}", exp)
        }
    }

    #[test]
    fn function_literal() {
        let input = "fn(x, y) { x + y; }";
        let prog = setup(input, 1);
        let exp = unwrap_expression(&prog);

        match exp {
            Expression::Function(func) => {
                assert_eq!(2, func.parameters.len(), "expected 2 parameters but got {:?}", func.parameters);
                assert_eq!(func.parameters.first().unwrap().name, "x");
                assert_eq!(func.parameters.last().unwrap().name, "y");
                assert_eq!(1, func.body.statements.len(), "expecte 1 body statement but got {:?}", func.body.statements);

                match func.body.statements.first().unwrap() {
                    Statement::Expression(stmt) => match &stmt.expression {
                        Expression::Infix(infix) => {
                            assert_eq!(infix.operator, Token::Plus, "expected + but got {}", infix.operator);
                            test_identifier(&infix.left, "x");
                            test_identifier(&infix.right, "y");
                        },
                        _ => panic!("expected infix expression but got {:?}", stmt.expression)
                    },
                    stmt => panic!("expected expression statement but got {:?}", stmt)
                }
            },
            _ => panic!("{} is not a function literal", exp)
        }
    }

    #[test]
    fn function_parameters() {
        struct Test<'a> {
            input: &'a str,
            expected_params: Vec<&'a str>,
        }

        let tests = vec![
            Test{input: "fn() {};", expected_params: vec![]},
            Test{input: "fn(x) {};", expected_params: vec!["x"]},
            Test{input: "fn(x, y, z) {};", expected_params: vec!["x", "y", "z"]},
        ];

        for t in tests {
            let prog = setup(t.input, 1);
            let exp = unwrap_expression(&prog);

            match exp {
                Expression::Function(func) => {
                    assert_eq!(func.parameters.len(), t.expected_params.len());
                    let mut params = t.expected_params.into_iter();
                    for param in &func.parameters {
                        let expected_param = params.next().unwrap();
                        assert_eq!(expected_param, param.name.as_str());
                    }
                },
                _ => panic!("{:?} not a function literal", exp)
            }
        }
    }

    #[test]
    fn call_expression() {
        let input = "add(1, 2 * 3, 4 + 5);";
        let prog = setup(input, 1);
        let exp = unwrap_expression(&prog);

        match exp {
            Expression::Call(call) => {
                test_identifier(&call.function, "add");
                assert_eq!(call.arguments.len(), 3);
                let mut args = (&call.arguments).into_iter();
                test_integer_literal(&args.next().unwrap(), 1);
                test_infix(&args.next().unwrap(), 2, Token::Asterisk, 3);
                test_infix(&args.next().unwrap(), 4, Token::Plus, 5)
            },
            _ => panic!("{} is not a call expression", exp)
        }
    }

    #[test]
    fn call_expression_parameter_parsing() {
        struct Test<'a> {
            input: &'a str,
            expected_ident: &'a str,
            expected_args: Vec<&'a str>,
        }

        let tests = vec![
            Test{input: "add();", expected_ident: "add", expected_args: vec![]},
            Test{input: "add(1);", expected_ident: "add", expected_args: vec!["1"]},
            Test{input: "add(1, 2 * 3, 4 + 5);", expected_ident: "add", expected_args: vec!["1", "(2 * 3)", "(4 + 5)"]},
        ];

        for t in tests {
            let prog = setup(t.input, 1);
            let exp = unwrap_expression(&prog);

            match exp {
                Expression::Call(call) => {
                    test_identifier(&call.function, t.expected_ident);
                    assert_eq!(call.arguments.len(), t.expected_args.len());
                    let mut args = (&call.arguments).into_iter();
                    for a in t.expected_args {
                        assert_eq!(a.to_string(), args.next().unwrap().to_string());
                    }
                },
                _ => panic!("{:?} is not a call expression", exp)
            }
        }
    }

    #[test]
    fn string_literal_expression() {
        let input = r#""hello world""#;
        let prog = setup(input, 1);
        let exp = unwrap_expression(&prog);

        match exp {
            Expression::String(s) => assert_eq!(s, "hello world"),
            _ => panic!("expected string literal but got {:?}", exp)
        }
    }

    #[test]
    fn array_literals() {
        let input = "[1, 2 * 2, 3 + 3]";
        let prog = setup(input, 1);
        let exp = unwrap_expression(&prog);

        match exp {
            Expression::Array(a) => {
                test_integer_literal(a.elements.first().unwrap(), 1);
                test_infix(a.elements.get(1).unwrap(), 2, Token::Asterisk, 2);
                test_infix(a.elements.last().unwrap(), 3, Token::Plus, 3);
            },
            _ => panic!("expected array literal but got {:?}", exp)
        }
    }

    #[test]
    fn index_expressions() {
        let input = "myArray[1 + 1]";
        let prog = setup(input, 1);
        let exp = unwrap_expression(&prog);

        match exp {
            Expression::Index(i) => {
                test_identifier(&i.left, "myArray");
                test_infix(&i.index, 1, Token::Plus, 1);
            },
            _ => panic!("expected an index expression but got {:?}", exp)
        }
    }

    #[test]
    fn hash_literals() {
        let input = r#"{"one": 1, "two": 2, "three": 3, 4: 4, true: true}"#;
        let prog = setup(input, 1);
        let exp = unwrap_expression(&prog);

        match exp {
            Expression::Hash(h) => {
                assert_eq!(h.pairs.len(), 5);

                for (k, v) in &h.pairs {
                    match (&k, &v) {
                        (Expression::String(key), Expression::Integer(int)) => {
                            match key.as_str() {
                                "one" => assert_eq!(1, *int),
                                "two" => assert_eq!(2, *int),
                                "three" => assert_eq!(3, *int),
                                _ => panic!("unexpected key {}", k)
                            }
                        },
                        (Expression::Integer(key), Expression::Integer(int)) => {
                            assert_eq!(*key, *int);
                            assert_eq!(*int, 4);
                        },
                        (Expression::Boolean(key), Expression::Boolean(val)) => {
                            assert_eq!(key, val)
                        },
                        _ => panic!("expected key to be a string and value to be an int but got {:?} and {:?}", k, v)
                    }
                }
            },
            _ => panic!("expected a hash literal but got {:?}", exp)
        }
    }

    #[test]
    fn hash_literal_with_expressions() {
        let input = r#"{"one": 0 + 1, "two": 10 - 8, "three": 15 / 5}"#;
        let prog = setup(input, 1);
        let exp = unwrap_expression(&prog);

        match exp {
            Expression::Hash(h) => {
                assert_eq!(h.pairs.len(), 3);

                for (k, v) in &h.pairs {
                    match (&k, &v) {
                        (Expression::String(key), Expression::Infix(_)) => {
                            match key.as_str() {
                                "one" => test_infix(v, 0, Token::Plus, 1),
                                "two" => test_infix(v, 10, Token::Minus, 8),
                                "three" => test_infix(v, 15, Token::Slash, 5),
                                _ => panic!("unexpected key {}", key)
                            }
                        },
                        _ => panic!("expected key to be a string and value to be an infix expression but got {:?} and {:?}", k, v)
                    }
                }
            },
            _ => panic!("expected a hash literal but got {:?}", exp)
        }
    }

    #[test]
    fn empty_hash_literal() {
        let input = "{}";
        let prog = setup(input, 1);
        let exp = unwrap_expression(&prog);

        match exp {
            Expression::Hash(h) => {
                assert_eq!(h.pairs.len(), 0)
            },
            _ => panic!("expected a hash literal but got {:?}", exp)
        }
    }

    fn test_infix(exp: &Expression, left: i64, op: Token, right: i64) {
        match exp {
            Expression::Infix(infix) => {
                assert_eq!(op, infix.operator, "expected {} operator but got {}", op, infix.operator);
                test_integer_literal(&infix.left, left);
                test_integer_literal(&infix.right, right);
            }
            exp => panic!("expected prefix expression but got {:?}", exp)
        }
    }

    fn test_if_condition(exp: &Expression, operator: Token, left: &str, right: &str) {
        match exp {
            Expression::Infix(infix) => {
                test_identifier(&infix.left, left);
                test_identifier(&infix.right, right);
                if operator != infix.operator {
                    panic!("expected {} operator but got {}", operator, infix.operator)
                }
            }
            _ => panic!("expected infix expression but got {:?}", exp)
        }
    }

    fn setup(input: &str, stmt_count: usize) -> Program {
        let l = Lexer::new(input);
        let mut p = Parser::new(l);
        let prog = p.parse_program().unwrap();

        if stmt_count != 0 && prog.statements.len() != stmt_count {
            panic!("expected 1 statement for '{}' but got {:?}", input, prog.statements)
        }

        prog
    }

    fn unwrap_expression(prog: &Program) -> &Expression {
        match prog.statements.first().unwrap() {
            Statement::Expression(stmt) => &stmt.expression,
            stmt => panic!("{:?} isn't an expression statement", stmt)
        }
    }

    fn test_integer_literal(exp: &Expression, value: i64) {
        match exp {
            Expression::Integer(int) => assert_eq!(value, *int, "expected {} but got {}", value, int),
            _ => panic!("expected integer literal {} but got {:?}", value, exp)
        }
    }

    fn test_boolean_literal(exp: &Expression, value: bool) {
        match exp {
            Expression::Boolean(val) => assert_eq!(value, *val, "expected {} but got {}", value, val),
            _ => panic!("expected boolean literal {} but got {:?}", value, exp)
        }
    }

    fn test_identifier(exp: &Expression, value: &str) {
        match exp {
            Expression::Identifier(ident) => assert_eq!(value, ident, "expected {} but got {}", value, ident),
            _ => panic!("expected identifier expression but got {:?}", exp)
        }
    }
}

