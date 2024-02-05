use nom::error::{ErrorKind, ParseError};
use nom_locate::LocatedSpan;

use crate::AccessorParserSpan;

#[derive(Clone, Copy, Debug)]
pub struct AccessorParserError {
    pub(crate) kind: AccessorParserErrorKind,
    pub(crate) span: AccessorParserSpan,
}

impl AccessorParserError {
    pub fn kind(&self) -> AccessorParserErrorKind {
        self.kind
    }

    pub fn span(&self) -> AccessorParserSpan {
        self.span
    }
}

#[derive(Clone, Copy, Debug)]
pub enum AccessorParserErrorKind {
    InvalidCharacter(char),
    InvalidEscapeCharacter(char),
    InvalidUnicode(InvalidUnicodeError),
    InvalidAccessorKey,
    MissingClosingBracket,
    InvalidAccessor,
    NotANumber,
    Unknown(ErrorKind),
}

#[derive(Clone, Copy, Debug)]
pub enum InvalidUnicodeError {
    MissingOpeningBracket,
    MissingClosingBracket,
    InvalidCodeLength,
    InvalidHexadecimal,
    InvalidCodePoint,
}

impl<'input> ParseError<LocatedSpan<&'input str>> for AccessorParserError {
    fn from_error_kind(input: LocatedSpan<&'input str>, kind: nom::error::ErrorKind) -> Self {
        let span_start = input.get_utf8_column();
        AccessorParserError {
            kind: AccessorParserErrorKind::Unknown(kind),
            span: AccessorParserSpan {
                start: span_start,
                end: span_start + 1,
            },
        }
    }

    fn append(_input: LocatedSpan<&'input str>, _kind: nom::error::ErrorKind, other: Self) -> Self {
        other
    }
}
