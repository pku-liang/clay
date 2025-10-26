use std::default;

use super::*;

use logos::{Logos, SpannedIter};

#[derive(Debug, thiserror::Error, miette::Diagnostic, Default, Clone, PartialEq)]
pub enum LexicalError {
    // #[error("invalid integer")]
    // InvalidInteger(#[from] std::num::ParseIntError),
    #[error("invalid radix integer")]
    InvalidRadixLit(String),
    #[error("invalid integer")]
    InvalidInteger(#[from] std::num::ParseIntError),
    #[default]
    #[error("invalid token")]
    InvalidToken
}

fn lit_from_lex(lex: &mut logos::Lexer<Token>) -> Result<TypedLit, LexicalError> {
    RadixIntLit::from_str(lex.slice())
    // TODO: why it used to be Type::UInt(0) here? Is Type::UInt(64) enough?
    .map(|lit| TypedLit::new(lit, Type::UInt(32).into()))
    .map_err(|err| LexicalError::InvalidRadixLit(err))
}

fn lit_with_width_from_lex(lex: &mut logos::Lexer<Token>) -> Result<TypedLit, LexicalError> {
    let v : Vec<_> = lex.slice().split("'").collect();
    let width: u32 = v[0].parse()?;
    let prefix = match v[1].chars().nth(0).unwrap() {
        'b' | 'B' => "0b",
        'o' | 'O' => "0o",
        'd' | 'D' => "",
        'h' | 'H' => "0x",
        _ => return Err(LexicalError::InvalidToken)
    };

    let s = prefix.to_string() + &v[1][1..];

    RadixIntLit::from_str(&s)
    .map(|lit| TypedLit::new(lit, Type::UInt(width).into()))
    .map_err(|err| LexicalError::InvalidRadixLit(err))
}


#[derive(Debug, thiserror::Error, miette::Diagnostic, Default, Clone, PartialEq)]
#[error("{cause}")]
#[diagnostic()]
pub struct SpannedLexicalError {
    cause: LexicalError,
    #[label("{cause}")]
    span: (usize, usize)
}


impl SpannedLexicalError {
    pub fn new(cause: LexicalError, span: (usize, usize)) -> Self {
        Self {
            cause,
            span
        }
    }
    pub fn span(&self) -> (usize, usize) {
        self.span
    }
}

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(error = LexicalError)]
#[logos(skip r"[ \t\n\f]+")]
#[logos(skip r"//[^\n\r]*[\n\r]*")]
#[logos(skip r"/\*[^*]*\*+(?:[^/*][^*]*\*+)*/")]
pub enum Token {
    // keywords
    #[token("var")]
    Var,
    #[token("rtype")]
    RType,
    #[token("mod")]
    Mod,
    #[token("flow")]
    Flow,
    #[token("let")]
    Let,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("while")]
    While,
    #[token("for")]
    For,
    #[token("return")]
    Return,
    #[token("break")]
    Break,
    #[token("continue")]
    Continue,
    #[token("match")]
    Match,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("regfile")]
    Regfile,
    #[token("state")]
    State,
    #[token("const")]
    Const,
    #[token("$signed")]
    SignedCast,
    #[token("$unsigned")]
    UnsignedCast,
    #[token("in")]
    In,
    #[token("mem")]
    Memory,
    #[token("memread")]
    MemRead,
    #[token("memwrite")]
    MemWrite,

    // operators
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Multiply,
    #[token("/")]
    Divide,
    #[token("%")]
    Remainder,
    #[token("==")]
    Equal,
    #[token("!=")]
    NotEqual,
    #[token("<")]
    LessThan,
    #[token("<=")]
    LessThanOrEqual,
    #[token(">")]
    GreaterThan,
    #[token(">=")]
    GreaterThanOrEqual,

    #[token("&&")]
    And,
    #[token("||")]
    Or,
    #[token("!")]
    Not,

    #[token("&")]
    BitAnd,
    #[token("|")]
    BitOr,
    #[token("^")]
    BitXor,
    #[token("~")]
    BitNot,

    #[token("<<")]
    LShift,
    #[token(">>")]
    RShift,


    #[token("=")]
    Assign,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,

    #[token("->")]
    Arrow,
    #[token("=>")]
    FatArrow,

    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token(";")]
    Semicolon,

    #[token("@")]
    At,
    #[token("#")]
    Hash,

    // identifiers
    #[regex("[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Identifier(String),

    // numbers
    #[regex("[0-9]+", lit_from_lex)]
    #[regex("0x[0-9a-fA-F]+", lit_from_lex)]
    #[regex("0b[01]+", lit_from_lex)]
    #[regex("0o[0-7]+", lit_from_lex)]
    #[regex("[0-9]+'[bB][01]+", lit_with_width_from_lex)]
    #[regex("[0-9]+'[oO][0-7]+", lit_with_width_from_lex)]
    #[regex("[0-9]+'[dD][0-9]+", lit_with_width_from_lex)]
    #[regex("[0-9]+'[hH][0-9a-fA-F]+", lit_with_width_from_lex)]
    Number(TypedLit),
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer() {
        let mut lexer = Token::lexer("mod let asdfasdf");

        assert_eq!(lexer.next(), Some(Ok(Token::Mod)));
        assert_eq!(lexer.next(), Some(Ok(Token::Let)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("asdfasdf".to_string()))));
    }
}

pub type Spanned<T,L,E> = std::result::Result<(L,T,L),E>;

pub struct Lexer<'input> {
    token_stream: SpannedIter<'input, Token>
}

impl<'input> Lexer<'input> {
    pub fn new(input: &'input str) -> Self {
        Self {
            token_stream: Token::lexer(input).spanned()
        }
    }
}

impl<'input> Iterator for Lexer<'input> {
    type Item = Spanned<Token, usize, SpannedLexicalError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.token_stream.next().map(|(token, span)| {
            Ok((span.start, token.map_err(
                |err| SpannedLexicalError::new(err, (span.start, span.end))
            )?, span.end))
        })
    }
}