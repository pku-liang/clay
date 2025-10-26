use super::*;
use miette::{SourceCode, SourceSpan};

use ast::lexer;


#[derive(thiserror::Error, miette::Diagnostic, fmt::Debug)]
pub enum Error {
    #[error("parse failed: {}", msg.replace("\n", ". "))]
    SpannedParseError {
        msg: String,
        #[label("here")]
        span: SourceSpan
    }
}

pub trait WithSourceCode<T> {
    fn with_source_code<S: SourceCode + Send + Sync + 'static>(self, source_code: S) -> Result<T, miette::Report>;
}

impl<T,E> WithSourceCode<T> for Result<T,E> 
where Error: From<E>
{
    fn with_source_code<S: SourceCode + Send + Sync + 'static>(self, source_code: S) -> Result<T, miette::Report> {
        self.map_err(Error::from)
        .map_err(miette::Error::from)
        .map_err(|err| {
            err.with_source_code(source_code)
        })
    }
}

impl From<
    lalrpop_util::ParseError<usize, lexer::Token, lexer::SpannedLexicalError>
> for Error {
    fn from(err: lalrpop_util::ParseError<usize, lexer::Token, lexer::SpannedLexicalError>) -> Self {
        use lalrpop_util::ParseError::*;
        let msg = err.to_string();
        // println!("from called, msg: {}", msg);
        let span = 
        match err {
            InvalidToken { location } | UnrecognizedEof { location, .. } => {
                SourceSpan::from(location)
            }
            UnrecognizedToken { token, .. } |
            ExtraToken { token } => {
                SourceSpan::from(token.0..token.2)
            }
            User { error } => {
                SourceSpan::from(error.span())
            }
        };
        Self::SpannedParseError { msg, span }
    }
}