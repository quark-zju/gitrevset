use crate::ast::Expr;
use lalrpop_util::lexer::Token;
use lalrpop_util::ParseError;

mod grammar;

/// Parse a string into an AST.
pub fn parse(s: &str) -> Result<Expr, ParseError<usize, Token, &str>> {
    grammar::ExprParser::new().parse(s)
}
