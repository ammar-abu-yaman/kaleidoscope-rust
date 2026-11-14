use std::ops::Range;

use lasso::Spur;
use strum::{Display, EnumDiscriminants};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Copy, Clone, PartialEq, Display, EnumDiscriminants)]
#[strum_discriminants(name(TokenTag), derive(Display, Hash))]
pub enum TokenKind {
    #[strum_discriminants(strum(to_string = "end of input"))]
    Eof,
    #[strum_discriminants(strum(to_string = "`def`"))]
    Def,
    #[strum_discriminants(strum(to_string = "`extern`"))]
    Extern,
    #[strum_discriminants(strum(to_string = "`(`"))]
    LParen,
    #[strum_discriminants(strum(to_string = "`)`"))]
    RParen,
    #[strum_discriminants(strum(to_string = "`,`"))]
    Comma,
    #[strum_discriminants(strum(to_string = "`;`"))]
    Semi,
    #[strum_discriminants(strum(to_string = "identifier"))]
    Ident(Symbol),
    #[strum_discriminants(strum(to_string = "number"))]
    Number(f64),
    // Any other ASCII punctuation; the parser decides whether it's an operator.
    #[strum_discriminants(strum(to_string = "operator"))]
    Op(char),
    // Text the lexer couldn't make sense of. A diagnostic has already been
    // reported for it, so later stages shouldn't report it again.
    #[strum_discriminants(strum(to_string = "invalid token"))]
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Symbol(pub Spur);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl From<Span> for Range<usize> {
    fn from(span: Span) -> Self {
        span.start as usize..span.end as usize
    }
}

impl Span {
    pub fn new(range: Range<usize>) -> Self {
        Self {
            start: range.start as u32,
            end: range.end as u32,
        }
    }

    pub fn to_range(self) -> Range<usize> {
        self.into()
    }
}
