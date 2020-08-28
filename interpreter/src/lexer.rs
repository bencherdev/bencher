use crate::token;
use crate::token::Token;
use std::iter::Peekable;
use std::str::Chars;

pub struct Lexer<'a> {
    input: Peekable<Chars<'a>>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Lexer<'_> {
        Lexer{
            input: input.chars().peekable(),
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.eat_whitespace();
        match self.read_char() {
            Some('=') => {
                if let Some('=') = self.peek_char() {
                    self.read_char();
                    Token::Eq
                } else {
                    Token::Assign
                }
            },
            Some('+') => Token::Plus,
            Some('(') => Token::Lparen,
            Some(')') => Token::Rparen,
            Some('{') => Token::Lbrace,
            Some('}') => Token::Rbrace,
            Some('[') => Token::Lbracket,
            Some(']') => Token::Rbracket,
            Some(',') => Token::Comma,
            Some(';') => Token::Semicolon,
            Some(':') => Token::Colon,
            Some('-') => Token::Minus,
            Some('!') => {
                if let Some('=') = self.peek_char() {
                    self.read_char();
                    Token::Neq
                } else {
                    Token::Bang
                }
            },
            Some('*') => Token::Asterisk,
            Some('/') => Token::Slash,
            Some('<') => Token::Lt,
            Some('>') => Token::Gt,
            Some(ch) => {
                if is_letter(ch) {
                    let ident = self.read_identifier(ch);
                    let tok = token::lookup_ident(ident);
                    tok
                } else if ch.is_digit(10) {
                    Token::Int(self.read_int(ch))
                } else if ch == '"' {
                    Token::String(self.read_string())
                } else {
                    Token::Illegal
                }
            },
            None => Token::EOF,
        }
    }

    fn read_string(&mut self) -> String {
        let mut str = String::new();
        while let Some(ch) = self.read_char() {
            if ch == '"' {
                return str;
            }
            str.push(ch);
        }
        str
    }

    fn eat_whitespace(&mut self) {
        while let Some(&ch) = self.input.peek() {
            if ch.is_whitespace() {
                self.read_char();
            } else {
                break;
            }
        }
    }

    fn read_char(&mut self) -> Option<char> {
        self.input.next()
    }

    fn peek_char(&mut self) -> Option<&char> {
        self.input.peek()
    }

    fn read_int(&mut self, ch: char) -> i64 {
        let mut s = String::new();
        s.push(ch);

        while let Some(&ch) = self.peek_char() {
            if ch.is_digit(10) {
                s.push(self.read_char().unwrap());
            } else {
                break;
            }
        }
        s.parse().unwrap()
    }

    fn read_identifier(&mut self, ch: char) -> String {
        let mut ident = String::new();
        ident.push(ch);

        while let Some(&ch) = self.peek_char() {
            if is_letter(ch) {
                ident.push(self.read_char().unwrap());
            } else {
                break;
            }
        }
        ident
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        let tok = self.next_token();
        if tok == Token::EOF {
            None
        } else {
            Some(tok)
        }
    }
}

fn is_letter(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_'
}

#[cfg(test)]
mod test {
    use crate::token::Token;
    use super::*;

    #[test]
    fn next_token() {
        let input = r#"let five = 5;
let ten = 10;

let add = fn(x, y) {
  x + y;
};

let result = add(five, ten);
!-/*5;
5 < 10 > 5;

if (5 < 10) {
    return true;
} else {
    return false;
}

10 == 10;
10 != 9;
"foobar"
"foo bar"
[1, 2];
{"foo": "bar"}"#;

        let tests = vec![
            Token::Let,
            Token::Ident("five".to_string()),
            Token::Assign,
            Token::Int(5),
            Token::Semicolon,
            Token::Let,
            Token::Ident("ten".to_string()),
            Token::Assign,
            Token::Int(10),
            Token::Semicolon,
            Token::Let,
            Token::Ident("add".to_string()),
            Token::Assign,
            Token::Function,
            Token::Lparen,
            Token::Ident("x".to_string()),
            Token::Comma,
            Token::Ident("y".to_string()),
            Token::Rparen,
            Token::Lbrace,
            Token::Ident("x".to_string()),
            Token::Plus,
            Token::Ident("y".to_string()),
            Token::Semicolon,
            Token::Rbrace,
            Token::Semicolon,
            Token::Let,
            Token::Ident("result".to_string()),
            Token::Assign,
            Token::Ident("add".to_string()),
            Token::Lparen,
            Token::Ident("five".to_string()),
            Token::Comma,
            Token::Ident("ten".to_string()),
            Token::Rparen,
            Token::Semicolon,
            Token::Bang,
            Token::Minus,
            Token::Slash,
            Token::Asterisk,
            Token::Int(5),
            Token::Semicolon,
            Token::Int(5),
            Token::Lt,
            Token::Int(10),
            Token::Gt,
            Token::Int(5),
            Token::Semicolon,
            Token::If,
            Token::Lparen,
            Token::Int(5),
            Token::Lt,
            Token::Int(10),
            Token::Rparen,
            Token::Lbrace,
            Token::Return,
            Token::True,
            Token::Semicolon,
            Token::Rbrace,
            Token::Else,
            Token::Lbrace,
            Token::Return,
            Token::False,
            Token::Semicolon,
            Token::Rbrace,
            Token::Int(10),
            Token::Eq,
            Token::Int(10),
            Token::Semicolon,
            Token::Int(10),
            Token::Neq,
            Token::Int(9),
            Token::Semicolon,
            Token::String("foobar".to_string()),
            Token::String("foo bar".to_string()),
            Token::Lbracket,
            Token::Int(1),
            Token::Comma,
            Token::Int(2),
            Token::Rbracket,
            Token::Semicolon,
            Token::Lbrace,
            Token::String("foo".to_string()),
            Token::Colon,
            Token::String("bar".to_string()),
            Token::Rbrace,
            Token::EOF,
        ];

        let mut l = Lexer::new(input);

        for t in tests.iter() {
            let tok = l.next_token();

            assert_eq!(*t, tok, "expected {} token but got {}", t, tok)
        }
    }
}