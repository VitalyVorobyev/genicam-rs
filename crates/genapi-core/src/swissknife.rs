use std::collections::HashSet;
use std::fmt;

/// Parsed SwissKnife expression represented as an abstract syntax tree.
#[derive(Debug, Clone)]
pub enum AstNode {
    /// Numeric literal stored as `f64`.
    Number(f64),
    /// Variable lookup resolved at evaluation time.
    Variable(String),
    /// Unary operator applied to a sub-expression.
    Unary {
        /// Operator kind (`+` or `-`).
        op: UnaryOp,
        /// Operand expression.
        expr: Box<AstNode>,
    },
    /// Binary operator combining two sub-expressions.
    Binary {
        /// Operator kind (`+`, `-`, `*`, `/`).
        op: BinaryOp,
        /// Left-hand side operand.
        left: Box<AstNode>,
        /// Right-hand side operand.
        right: Box<AstNode>,
    },
}

/// Binary operator kinds supported by the SwissKnife subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
}

/// Unary operator kinds supported by the SwissKnife subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Plus,
    Minus,
}

/// Error produced while parsing a SwissKnife expression.
#[derive(Debug, Clone)]
pub struct ParseError {
    msg: String,
}

impl ParseError {
    fn new<S: Into<String>>(msg: S) -> Self {
        Self { msg: msg.into() }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl std::error::Error for ParseError {}

/// Error produced while evaluating a SwissKnife expression.
#[derive(Debug, Clone)]
pub enum EvalError {
    /// Variable referenced by the expression has no bound value.
    UnknownVariable(String),
    /// Division by zero occurred while evaluating `/`.
    DivisionByZero,
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::UnknownVariable(var) => write!(f, "unknown variable {var}"),
            EvalError::DivisionByZero => write!(f, "division by zero"),
        }
    }
}

impl std::error::Error for EvalError {}

/// Parse an arithmetic SwissKnife expression into an [`AstNode`].
pub fn parse_expression(input: &str) -> Result<AstNode, ParseError> {
    let mut parser = Parser::new(input)?;
    let expr = parser.parse_expr()?;
    if !matches!(parser.lookahead, Token::End) {
        return Err(ParseError::new("unexpected trailing tokens"));
    }
    Ok(expr)
}

/// Evaluate an [`AstNode`] using the provided variable resolver.
///
/// The resolver receives variable identifiers and must return their numeric
/// value. Returning [`EvalError::UnknownVariable`] is propagated to the caller.
pub fn evaluate(
    ast: &AstNode,
    vars: &mut dyn FnMut(&str) -> Result<f64, EvalError>,
) -> Result<f64, EvalError> {
    match ast {
        AstNode::Number(value) => Ok(*value),
        AstNode::Variable(name) => vars(name),
        AstNode::Unary { op, expr } => {
            let inner = evaluate(expr, vars)?;
            match op {
                UnaryOp::Plus => Ok(inner),
                UnaryOp::Minus => Ok(-inner),
            }
        }
        AstNode::Binary { op, left, right } => {
            let lhs = evaluate(left, vars)?;
            let rhs = evaluate(right, vars)?;
            match op {
                BinaryOp::Add => Ok(lhs + rhs),
                BinaryOp::Sub => Ok(lhs - rhs),
                BinaryOp::Mul => Ok(lhs * rhs),
                BinaryOp::Div => {
                    if rhs == 0.0 {
                        return Err(EvalError::DivisionByZero);
                    }
                    Ok(lhs / rhs)
                }
            }
        }
    }
}

/// Collect all variable identifiers referenced by the AST.
pub fn collect_identifiers(ast: &AstNode, out: &mut HashSet<String>) {
    match ast {
        AstNode::Number(_) => {}
        AstNode::Variable(name) => {
            out.insert(name.clone());
        }
        AstNode::Unary { expr, .. } => collect_identifiers(expr, out),
        AstNode::Binary { left, right, .. } => {
            collect_identifiers(left, out);
            collect_identifiers(right, out);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    End,
}

struct Lexer<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Lexer {
            input: input.as_bytes(),
            pos: 0,
        }
    }

    fn next_token(&mut self) -> Result<Token, ParseError> {
        self.skip_ws();
        let Some(&byte) = self.input.get(self.pos) else {
            return Ok(Token::End);
        };
        match byte {
            b'0'..=b'9' | b'.' => self.lex_number(),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.lex_ident(),
            b'+' => {
                self.pos += 1;
                Ok(Token::Plus)
            }
            b'-' => {
                self.pos += 1;
                Ok(Token::Minus)
            }
            b'*' => {
                self.pos += 1;
                Ok(Token::Star)
            }
            b'/' => {
                self.pos += 1;
                Ok(Token::Slash)
            }
            b'(' => {
                self.pos += 1;
                Ok(Token::LParen)
            }
            b')' => {
                self.pos += 1;
                Ok(Token::RParen)
            }
            _ => Err(ParseError::new(format!(
                "unexpected character '{}'",
                byte as char
            ))),
        }
    }

    fn skip_ws(&mut self) {
        while let Some(&byte) = self.input.get(self.pos) {
            if byte.is_ascii_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn lex_number(&mut self) -> Result<Token, ParseError> {
        let start = self.pos;
        let mut seen_digit = false;
        let mut seen_dot = false;
        while let Some(&byte) = self.input.get(self.pos) {
            match byte {
                b'0'..=b'9' => {
                    seen_digit = true;
                    self.pos += 1;
                }
                b'.' if !seen_dot => {
                    seen_dot = true;
                    self.pos += 1;
                }
                b'.' => break,
                _ => break,
            }
        }
        if !seen_digit {
            return Err(ParseError::new("invalid number literal"));
        }
        let slice = &self.input[start..self.pos];
        let text =
            std::str::from_utf8(slice).map_err(|_| ParseError::new("invalid UTF-8 in number"))?;
        let value = text
            .parse::<f64>()
            .map_err(|_| ParseError::new("failed to parse number"))?;
        Ok(Token::Number(value))
    }

    fn lex_ident(&mut self) -> Result<Token, ParseError> {
        let start = self.pos;
        self.pos += 1;
        while let Some(&byte) = self.input.get(self.pos) {
            if byte.is_ascii_alphanumeric() || byte == b'_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let slice = &self.input[start..self.pos];
        let text = std::str::from_utf8(slice)
            .map_err(|_| ParseError::new("invalid UTF-8 in identifier"))?;
        Ok(Token::Ident(text.to_string()))
    }
}

struct Parser<'a> {
    lexer: Lexer<'a>,
    lookahead: Token,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new(input);
        let lookahead = lexer.next_token()?;
        Ok(Parser { lexer, lookahead })
    }

    fn parse_expr(&mut self) -> Result<AstNode, ParseError> {
        let mut node = self.parse_term()?;
        loop {
            match self.lookahead {
                Token::Plus => {
                    self.advance()?;
                    let rhs = self.parse_term()?;
                    node = AstNode::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(node),
                        right: Box::new(rhs),
                    };
                }
                Token::Minus => {
                    self.advance()?;
                    let rhs = self.parse_term()?;
                    node = AstNode::Binary {
                        op: BinaryOp::Sub,
                        left: Box::new(node),
                        right: Box::new(rhs),
                    };
                }
                _ => break,
            }
        }
        Ok(node)
    }

    fn parse_term(&mut self) -> Result<AstNode, ParseError> {
        let mut node = self.parse_factor()?;
        loop {
            match self.lookahead {
                Token::Star => {
                    self.advance()?;
                    let rhs = self.parse_factor()?;
                    node = AstNode::Binary {
                        op: BinaryOp::Mul,
                        left: Box::new(node),
                        right: Box::new(rhs),
                    };
                }
                Token::Slash => {
                    self.advance()?;
                    let rhs = self.parse_factor()?;
                    node = AstNode::Binary {
                        op: BinaryOp::Div,
                        left: Box::new(node),
                        right: Box::new(rhs),
                    };
                }
                _ => break,
            }
        }
        Ok(node)
    }

    fn parse_factor(&mut self) -> Result<AstNode, ParseError> {
        match self.lookahead.clone() {
            Token::Plus => {
                self.advance()?;
                let expr = self.parse_factor()?;
                Ok(AstNode::Unary {
                    op: UnaryOp::Plus,
                    expr: Box::new(expr),
                })
            }
            Token::Minus => {
                self.advance()?;
                let expr = self.parse_factor()?;
                Ok(AstNode::Unary {
                    op: UnaryOp::Minus,
                    expr: Box::new(expr),
                })
            }
            Token::Number(value) => {
                self.advance()?;
                Ok(AstNode::Number(value))
            }
            Token::Ident(name) => {
                self.advance()?;
                Ok(AstNode::Variable(name))
            }
            Token::LParen => {
                self.advance()?;
                let expr = self.parse_expr()?;
                if !matches!(self.lookahead, Token::RParen) {
                    return Err(ParseError::new("missing closing ')'"));
                }
                self.advance()?;
                Ok(expr)
            }
            Token::End => Err(ParseError::new("unexpected end of expression")),
            other => Err(ParseError::new(format!("unexpected token {other:?}"))),
        }
    }

    fn advance(&mut self) -> Result<(), ParseError> {
        self.lookahead = self.lexer.next_token()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::trace;

    #[test]
    fn parse_basic_expression() {
        let expr = "(A + 2) * 3 - B / 4";
        let ast = parse_expression(expr).expect("parse expression");
        trace!(?ast, "parsed ast");
        let mut vars = |name: &str| match name {
            "A" => Ok(4.0),
            "B" => Ok(8.0),
            _ => Err(EvalError::UnknownVariable(name.to_string())),
        };
        let value = evaluate(&ast, &mut vars).expect("eval");
        assert!((value - 16.0).abs() < 1e-6);
    }

    #[test]
    fn parse_unary_and_precedence() {
        let expr = "-A + 10 / (B - 5)";
        let ast = parse_expression(expr).expect("parse unary");
        trace!(?ast, "parsed ast unary");
        let mut vars = |name: &str| match name {
            "A" => Ok(3.0),
            "B" => Ok(7.0),
            _ => Err(EvalError::UnknownVariable(name.to_string())),
        };
        let value = evaluate(&ast, &mut vars).expect("eval");
        assert!((value - 2.0).abs() < 1e-6);
    }

    #[test]
    fn division_by_zero_error() {
        let ast = parse_expression("A / B").expect("parse");
        let mut vars = |name: &str| match name {
            "A" => Ok(5.0),
            "B" => Ok(0.0),
            _ => Err(EvalError::UnknownVariable(name.to_string())),
        };
        let err = evaluate(&ast, &mut vars).expect_err("division by zero");
        assert!(matches!(err, EvalError::DivisionByZero));
    }
}
