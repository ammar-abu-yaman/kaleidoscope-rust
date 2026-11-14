use lasso::Rodeo;

use crate::diagnostic::{Diagnostic, Diagnostics};
use crate::token::{Span, Symbol, Token, TokenKind};

pub fn lex(src: &str, interner: &mut Rodeo) -> (TokenStream, Diagnostics) {
    Lexer::new(src, interner).run()
}

struct Lexer<'src, 'i> {
    src: &'src str,
    pos: usize,
    interner: &'i mut Rodeo,
    diagnostics: Diagnostics,
}

impl<'src, 'i> Lexer<'src, 'i> {
    fn new(src: &'src str, interner: &'i mut Rodeo) -> Self {
        Self {
            src,
            pos: 0,
            interner,
            diagnostics: Diagnostics::new(),
        }
    }

    fn run(mut self) -> (TokenStream, Diagnostics) {
        let mut tokens = vec![];
        loop {
            let token = self.next_token();
            tokens.push(token);
            if matches!(
                token,
                Token {
                    kind: TokenKind::Eof,
                    ..
                }
            ) {
                break;
            }
        }
        (TokenStream::new(tokens), self.diagnostics)
    }

    // Consumes the first char before dispatching, so every branch makes
    // progress and the lexer can't get stuck on the same position.
    fn next_token(&mut self) -> Token {
        self.skip_trivia();
        let start = self.pos;
        let kind = match self.bump() {
            None => TokenKind::Eof,
            Some(c) if c.is_ascii_digit() => self.number(start),
            Some('.') if self.peek().is_some_and(|c| c.is_ascii_digit()) => self.number(start),
            Some(c) if c.is_ascii_alphabetic() => self.ident_or_keyword(start),
            Some('(') => TokenKind::LParen,
            Some(')') => TokenKind::RParen,
            Some(',') => TokenKind::Comma,
            Some(';') => TokenKind::Semi,
            Some(c) if c.is_ascii_punctuation() => TokenKind::Op(c),
            Some(_) => self.invalid(start),
        };
        Token {
            kind,
            span: self.span_from(start),
        }
    }

    fn ident_or_keyword(&mut self, start: usize) -> TokenKind {
        self.eat_while(Self::is_alphanum);
        match self.text(start) {
            "def" => TokenKind::Def,
            "extern" => TokenKind::Extern,
            ident => TokenKind::Ident(self.intern(ident)),
        }
    }

    fn number(&mut self, start: usize) -> TokenKind {
        loop {
            match self.peek() {
                Some(c) if c.is_ascii_digit() => self.bump(),
                Some('.') if self.peek_nth(1).is_some_and(|c| c.is_ascii_digit()) => self.bump(),
                _ => break,
            };
        }
        let lexeme = self.text(start);
        match lexeme.parse() {
            Ok(num) => TokenKind::Number(num),
            Err(_) => {
                // Only a second `.` can make this fail; keep a best-effort
                // value so the parser carries on without a follow-up error.
                let end = lexeme
                    .match_indices('.')
                    .nth(1)
                    .map_or(lexeme.len(), |(i, _)| i);
                let diagnostic = Diagnostic::error(
                    self.span_from(start),
                    format!("invalid number literal `{lexeme}`"),
                )
                .with_primary_label("a number can contain at most one `.`");
                self.diagnostics.push(diagnostic);
                TokenKind::Number(lexeme[..end].parse().unwrap_or(0.0))
            }
        }
    }

    // Groups a run of unrecognized chars into one token and one diagnostic.
    fn invalid(&mut self, start: usize) -> TokenKind {
        self.eat_while(Self::is_invalid);
        let span = self.span_from(start);
        let message = match self.text(start).chars().count() {
            1 => format!("unexpected character `{}`", self.text(start)),
            _ => format!("unexpected characters `{}`", self.text(start)),
        };
        self.error(span, message);
        TokenKind::Error
    }

    fn peek(&self) -> Option<char> {
        self.peek_nth(0)
    }

    fn peek_nth(&self, n: usize) -> Option<char> {
        self.src[self.pos..].chars().nth(n)
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn eat_while(&mut self, pred: impl Fn(char) -> bool) {
        while self.peek().is_some_and(&pred) {
            self.bump();
        }
    }

    // Borrows from the source, not the lexer, so it can be held across
    // calls that need `&mut self`.
    fn text(&self, start: usize) -> &'src str {
        &self.src[start..self.pos]
    }

    fn span_from(&self, start: usize) -> Span {
        Span::new(start..self.pos)
    }

    fn error(&mut self, span: Span, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::error(span, message));
    }

    fn skip_trivia(&mut self) {
        loop {
            self.eat_while(Self::is_whitespace);
            if self.peek() == Some('#') {
                self.eat_while(Self::is_not_newline);
            } else {
                break;
            }
        }
    }

    fn intern(&mut self, text: &str) -> Symbol {
        Symbol(self.interner.get_or_intern(text))
    }

    fn is_whitespace(c: char) -> bool {
        c.is_whitespace()
    }

    fn is_not_newline(c: char) -> bool {
        c != '\n'
    }

    fn is_alphanum(c: char) -> bool {
        c.is_ascii_alphanumeric()
    }

    fn is_invalid(c: char) -> bool {
        !(c.is_ascii_alphanumeric() || c.is_ascii_punctuation() || c.is_whitespace())
    }
}

pub struct TokenStream {
    tokens: Vec<Token>,
}

impl TokenStream {
    pub(crate) fn new(tokens: Vec<Token>) -> Self {
        debug_assert!(
            matches!(
                tokens.last(),
                Some(Token {
                    kind: TokenKind::Eof,
                    ..
                })
            ),
            "token stream must end with Eof"
        );
        Self { tokens }
    }

    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex_str(src: &str) -> (Vec<TokenKind>, Vec<String>, Rodeo) {
        let mut interner = Rodeo::default();
        let (tokens, diagnostics) = lex(src, &mut interner);
        let kinds = tokens.tokens().iter().map(|t| t.kind).collect();
        let messages = diagnostics.iter().map(|d| d.message.clone()).collect();
        (kinds, messages, interner)
    }

    fn ident(interner: &Rodeo, name: &str) -> TokenKind {
        TokenKind::Ident(Symbol(interner.get(name).expect("name was not interned")))
    }

    #[test]
    fn empty_input_is_just_eof() {
        let (kinds, messages, _) = lex_str("");
        assert_eq!(kinds, [TokenKind::Eof]);
        assert!(messages.is_empty());
    }

    #[test]
    fn keywords_identifiers_and_punctuation() {
        let (kinds, messages, interner) = lex_str("def foo(x, y) x+y;");
        let (foo, x, y) = (
            ident(&interner, "foo"),
            ident(&interner, "x"),
            ident(&interner, "y"),
        );
        assert_eq!(
            kinds,
            [
                TokenKind::Def,
                foo,
                TokenKind::LParen,
                x,
                TokenKind::Comma,
                y,
                TokenKind::RParen,
                x,
                TokenKind::Op('+'),
                y,
                TokenKind::Semi,
                TokenKind::Eof,
            ]
        );
        assert!(messages.is_empty());
    }

    #[test]
    fn numbers() {
        let (kinds, messages, _) = lex_str("42 3.25 .5");
        assert_eq!(
            kinds,
            [
                TokenKind::Number(42.0),
                TokenKind::Number(3.25),
                TokenKind::Number(0.5),
                TokenKind::Eof
            ]
        );
        assert!(messages.is_empty());
    }

    #[test]
    fn trailing_dot_is_not_part_of_number() {
        let (kinds, _, _) = lex_str("1.");
        assert_eq!(
            kinds,
            [TokenKind::Number(1.0), TokenKind::Op('.'), TokenKind::Eof]
        );
    }

    #[test]
    fn lone_dot_is_an_operator() {
        let (kinds, _, _) = lex_str(".");
        assert_eq!(kinds, [TokenKind::Op('.'), TokenKind::Eof]);
    }

    #[test]
    fn malformed_number_reports_once_and_keeps_going() {
        let (kinds, messages, _) = lex_str("1.2.3 extern");
        assert_eq!(
            kinds,
            [TokenKind::Number(1.2), TokenKind::Extern, TokenKind::Eof]
        );
        assert_eq!(messages, ["invalid number literal `1.2.3`"]);
    }

    #[test]
    fn comments_and_whitespace_are_skipped() {
        let (kinds, messages, _) = lex_str("# one\n  # two\n\tdef # trailing");
        assert_eq!(kinds, [TokenKind::Def, TokenKind::Eof]);
        assert!(messages.is_empty());
    }

    #[test]
    fn non_ascii_is_one_error_token_not_a_panic() {
        let (kinds, messages, interner) = lex_str("caf\u{e9}\u{e9} x");
        assert_eq!(
            kinds,
            [
                ident(&interner, "caf"),
                TokenKind::Error,
                ident(&interner, "x"),
                TokenKind::Eof
            ]
        );
        assert_eq!(messages, ["unexpected characters `\u{e9}\u{e9}`"]);
    }

    #[test]
    fn spans_are_byte_ranges_and_exclude_trivia() {
        let mut interner = Rodeo::default();
        let (tokens, _) = lex("  def \u{e9}x", &mut interner);
        let spans: Vec<_> = tokens.tokens().iter().map(|t| t.span.to_range()).collect();
        // `é` is two bytes, so the identifier after it starts at byte 8.
        assert_eq!(spans, [2..5, 6..8, 8..9, 9..9]);
    }
}
