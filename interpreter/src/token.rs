use std::fmt;

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub enum Token {
    Illegal,
    EOF,

    // Identifiers + literals
    Ident(String),
    Int(i64),
    String(String),

    // Operators
    Assign,
    Plus,
    Minus,
    Bang,
    Asterisk,
    Slash,

    Lt,
    Gt,
    Eq,
    Neq,

    // Delimiters
    Comma,
    Semicolon,
    Colon,

    Lparen,
    Rparen,
    Lbrace,
    Rbrace,
    Lbracket,
    Rbracket,

    // Keywords
    Function,
    Let,
    True,
    False,
    If,
    Else,
    Return,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Ident(name) => write!(f, "{}", name),
            Token::Int(val) => write!(f, "{}", val),
            Token::Minus => write!(f, "-"),
            Token::Plus => write!(f, "+"),
            Token::Bang => write!(f, "!"),
            Token::Asterisk => write!(f, "*"),
            Token::Slash => write!(f, "/"),
            Token::Gt => write!(f, ">"),
            Token::Lt => write!(f, "<"),
            Token::Eq => write!(f, "=="),
            Token::Neq => write!(f, "!="),
            Token::Semicolon => write!(f, ";"),
            Token::Colon => write!(f, ":"),
            Token::Assign => write!(f, "="),
            Token::Function => write!(f, "fn"),
            Token::Lparen => write!(f, "("),
            Token::Rparen => write!(f, ")"),
            Token::Lbrace => write!(f, "{{"),
            Token::Rbrace => write!(f, "}}"),
            Token::Comma => write!(f, ","),
            Token::Lbracket => write!(f, "["),
            Token::Rbracket => write!(f, "]"),
            tok => write!(f, "{:?}", tok),
        }
    }
}

pub fn lookup_ident(ident: String) -> Token {
    match ident.as_str() {
        "let" => Token::Let,
        "fn" => Token::Function,
        "true" => Token::True,
        "false" => Token::False,
        "if" => Token::If,
        "else" => Token::Else,
        "return" => Token::Return,
        _ => Token::Ident(ident),
    }
}

impl Token {
    pub fn is_ident(&self) -> bool {
        match self {
            Token::Ident(_) => true,
            _ => false,
        }
    }

    pub fn is_int(&self) -> bool {
        match self {
            Token::Int(_) => true,
            _ => false,
        }
    }
}