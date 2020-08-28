use crate::token;
use std::fmt;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

#[derive(Debug)]
pub enum Node {
    Program(Box<Program>),
    Statement(Box<Statement>),
    Expression(Box<Expression>),
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub enum Statement {
    Let(Box<LetStatement>),
    Return(Box<ReturnStatement>),
    Expression(Box<ExpressionStatement>),
}

impl fmt::Display for Statement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Statement::Let(stmt) => format!("{}", stmt),
            Statement::Return(ret) => format!("{}", ret),
            Statement::Expression(exp) => format!("{}", exp),
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Expression {
    Identifier(String),
    Integer(i64),
    Prefix(Box<PrefixExpression>),
    Infix(Box<InfixExpression>),
    Boolean(bool),
    String(String),
    If(Box<IfExpression>),
    Function(Box<FunctionLiteral>),
    Call(Box<CallExpression>),
    Array(Box<ArrayLiteral>),
    Index(Box<IndexExpression>),
    Hash(Box<HashLiteral>),
}

impl Expression {
    pub fn string(&self) -> String {
        match self {
            Expression::Identifier(s) => s.clone(),
            Expression::Integer(value) => format!("{}", value),
            Expression::Prefix(pref) => pref.to_string(),
            Expression::Infix(infix) => infix.to_string(),
            Expression::Boolean(b) => b.to_string(),
            Expression::String(s) => s.clone(),
            Expression::If(exp) => exp.to_string(),
            Expression::Function(f) => f.to_string(),
            Expression::Call(c) => c.to_string(),
            Expression::Array(a) => a.to_string(),
            Expression::Index(i) => i.to_string(),
            Expression::Hash(h) => h.to_string(),
        }
    }
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.string())
    }
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct HashLiteral {
    pub pairs: HashMap<Expression,Expression>,
}

// Had to implement Hash for this because HashMap doesn't. Doesn't matter what this is because
// a HashLiteral isn't a valid expression as a key in a monkey hash.
impl Hash for HashLiteral {
    fn hash<H: Hasher>(&self, _state: &mut H) {
        panic!("hash not implemented for HashLiteral");
    }
}

impl fmt::Display for HashLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pairs: Vec<String> = (&self.pairs).into_iter().map(|(k, v)| format!("{}:{}", k.to_string(), v.to_string())).collect();
        write!(f, "{{{}}}", pairs.join(", "))
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct IfExpression {
    pub condition: Expression,
    pub consequence: BlockStatement,
    pub alternative: Option<BlockStatement>,
}

impl fmt::Display for IfExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "if {} {}", self.condition, self.consequence)?;

        if let Some(ref stmt) = self.alternative {
            write!(f, "else {}", stmt)?;
        }
        Ok(())
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct ArrayLiteral {
    pub elements: Vec<Expression>,
}

impl fmt::Display for ArrayLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let elements: Vec<String> = (&self.elements).into_iter().map(|e| e.to_string()).collect();
        write!(f, "[{}]", elements.join(", "))
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct IndexExpression {
    pub left: Expression,
    pub index: Expression,
}

impl fmt::Display for IndexExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}[{}])", self.left.to_string(), self.index.to_string())
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct BlockStatement {
    pub statements: Vec<Statement>,
}

impl fmt::Display for BlockStatement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for stmt in &self.statements {
            write!(f, "{}", stmt)?;
        }
        Ok(())
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct FunctionLiteral {
    pub parameters: Vec<IdentifierExpression>,
    pub body: BlockStatement,
}

impl fmt::Display for FunctionLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let param_list: Vec<String> = (&self.parameters).into_iter().map(|p| p.to_string()).collect();
        write!(f, "({}) {}", param_list.join(", "), self.body)
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct CallExpression {
    pub function: Expression,
    pub arguments: Vec<Expression>,
}

impl fmt::Display for CallExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let arg_list: Vec<String> = (&self.arguments).into_iter().map(|exp| exp.to_string()).collect();
        write!(f, "{}({})", self.function.to_string(), arg_list.join(", "))
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct IdentifierExpression {
    pub name: String,
}

impl fmt::Display for IdentifierExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct PrefixExpression {
    pub operator: token::Token,
    pub right: Expression,
}

impl fmt::Display for PrefixExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}{})", self.operator, self.right)
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct InfixExpression {
    pub operator: token::Token,
    pub left: Expression,
    pub right: Expression,
}

impl fmt::Display for InfixExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({} {} {})", self.left, self.operator, self.right)
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct IntegerLiteral {
    pub value: i64,
}

impl fmt::Display for IntegerLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct LetStatement {
    pub name: String,
    pub value: Expression,
}

impl fmt::Display for LetStatement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "let {} = {};", self.name, self.value)
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct ReturnStatement {
    pub value: Expression,
}

impl fmt::Display for ReturnStatement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "return {};", self.value)
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct ExpressionStatement {
    pub expression: Expression
}

impl fmt::Display for ExpressionStatement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.expression)
    }
}

#[derive(Debug)]
pub struct Program {
    pub statements: Vec<Statement>,
}

impl Program {
    pub fn new() -> Program {
        Program {
            statements: Vec::new(),
        }
    }
}

impl fmt::Display for Program {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let statements: Vec<String> = (&self.statements).into_iter().map(|stmt| stmt.to_string()).collect();
        write!(f, "{}", statements.join(""))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn display() {

        let p = Program{
            statements: vec![
                Statement::Let(Box::new(
                    LetStatement{
                            name: "asdf".to_string(),
                            value: Expression::Identifier("bar".to_string())}))],
        };

        let expected = "let asdf = bar;";

        if p.to_string() != expected {
            panic!("expected {} but got {}", "foo", expected)
        }
    }
}
