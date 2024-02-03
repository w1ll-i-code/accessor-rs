use nom::{
    branch::alt,
    bytes::complete::{tag, take_until},
    character::complete::anychar,
    error::Error,
    sequence::{preceded, terminated},
    Err,
};
use nom_locate::LocatedSpan;

use crate::error::{
    AccessorParserError, AccessorParserErrorKind, AccessorParserErrorSpan, InvalidUnicodeError,
};

const RESERVED_TOKEN: &[char] = &['\\', '{', '}', '[', ']', '.', '$'];

type PResult<'input, Output> = Result<(LocatedSpan<&'input str>, Output), Err<AccessorParserError>>;
type NomError<'input> = Error<LocatedSpan<&'input str>>;

#[derive(Clone, Debug)]
pub struct Accessor {
    keys: Box<[AccessorKey]>,
}

#[derive(Clone, Debug)]
enum AccessorKey {
    String(Box<str>),
    Numeric(usize),
}

impl From<String> for AccessorKey {
    fn from(value: String) -> Self {
        AccessorKey::String(value.into_boxed_str())
    }
}

impl From<usize> for AccessorKey {
    fn from(value: usize) -> Self {
        AccessorKey::Numeric(value)
    }
}

pub fn take_accessor<'input>(input: LocatedSpan<&'input str>) -> PResult<'input, Option<Accessor>> {
    let Ok((input, opening)) = tag::<_, _, NomError>("${")(input) else {
        return Ok((input, None));
    };

    dbg!(&opening);

    let (input, root) = take_string_until(is_separator)(input)?;

    let mut keys = vec![AccessorKey::from(root)];
    let mut input = input;

    let error = loop {
        match take_key(input) {
            Ok((next, key)) => {
                keys.push(key);
                input = next;
            }
            Err(err @ Err::Failure(_)) => return Err(err),
            Err(err) => break err,
        }
    };

    dbg!(&keys);
    dbg!(&input);

    let Ok((input, _)) = tag::<_, _, NomError>("}")(input) else {
        let span_start = opening.get_utf8_column() - 1;
        return Err(Err::Failure(AccessorParserError {
            kind: AccessorParserErrorKind::MissingClosingBracket,
            span: AccessorParserErrorSpan {
                start: span_start,
                end: span_start + 2
            }
        }));
    };

    Ok((
        input,
        Some(Accessor {
            keys: keys.into_boxed_slice(),
        }),
    ))
}

fn take_key<'input>(input: LocatedSpan<&'input str>) -> PResult<'input, AccessorKey> {
    alt((take_string_key, take_index_key))(input)
}

fn take_index_key<'input>(input: LocatedSpan<&'input str>) -> PResult<'input, AccessorKey> {
    let Ok((input, opening_bracket)) = tag::<_, _, NomError>("[")(input) else {
        let span_start = input.get_utf8_column() - 1;
        let next_separator = find_next_separator(input);
        let span_end = input.fragment()[..next_separator].chars().count();

        return Err(Err::Error(AccessorParserError {
            kind: AccessorParserErrorKind::InvalidAccessor,
            span: AccessorParserErrorSpan {
                start: span_start,
                end: span_end,
            },
        }));
    };

    let Ok((input, index)) = terminated(take_until("]"), tag::<_, _, NomError>("]"))(input) else {
        let span_start = opening_bracket.get_utf8_column() - 1;
        return Err(Err::Failure(AccessorParserError {
            kind: AccessorParserErrorKind::MissingClosingBracket,
            span: AccessorParserErrorSpan {
                start: span_start,
                end: span_start + 1,
            },
        }));
    };

    let Some(index): Option<usize> = index.parse().ok() else {
        let span_start = input.get_utf8_column() - 1;
        let span_end = span_start + index.chars().count();
        return Err(Err::Failure(AccessorParserError {
            kind: AccessorParserErrorKind::NotANumber,
            span: AccessorParserErrorSpan {
                start: span_start,
                end: span_end,
            },
        }));
    };

    Ok((input, index.into()))
}

fn take_string_key<'input>(input: LocatedSpan<&'input str>) -> PResult<'input, AccessorKey> {
    let Ok((input, _)) = tag::<_, _, NomError>(".")(input) else {
        let span_start = input.get_utf8_column() - 1;
        let next_separator = find_next_separator(input);
        let span_end = input.fragment()[..next_separator].chars().count();

        return Err(Err::Error(AccessorParserError {
            kind: AccessorParserErrorKind::InvalidAccessor,
            span: AccessorParserErrorSpan {
                start: span_start,
                end: span_end,
            },
        }));
    };

    match take_string_until(is_separator)(input) {
        Ok((rest, string)) => Ok((rest, string.into())),
        Err(err) => Err(err),
    }
}

fn find_next_separator(input: LocatedSpan<&str>) -> usize {
    match take_string_until(is_separator)(input) {
        Ok((rest, _)) => input.len() - rest.len(),
        Result::Err(_) => input.len(),
    }
}

fn is_separator(c: char) -> bool {
    c == '.' || c == '[' || c == '}'
}

fn take_string<'input>(input: LocatedSpan<&'input str>) -> PResult<'input, String> {
    take_string_until(|c| c == '$')(input)
}

fn take_string_until<'input, Cond: Fn(char) -> bool>(
    cond: Cond,
) -> impl Fn(LocatedSpan<&'input str>) -> PResult<'input, String> {
    move |mut input| {
        let mut buf = String::new();
        loop {
            let Ok((rest, ch)) = anychar::<_, NomError>(input) else {
                return Ok((input, buf));
            };

            if cond(ch) {
                return Ok((input, buf));
            }

            let (rest, ch) = alt((preceded(tag("\\"), take_escaped_char), take_char))(input)?;

            input = rest;
            buf.push(ch);
        }
    }
}

fn take_escaped_char<'input>(input: LocatedSpan<&'input str>) -> PResult<'input, char> {
    let (rest, ch) = anychar(input)?;
    match ch {
        '\\' | '{' | '}' | '[' | ']' | '.' | '$' => Ok((rest, ch)),
        'n' => Ok((rest, '\n')),
        't' => Ok((rest, '\t')),
        'r' => Ok((rest, '\r')),
        'u' => take_unicode(rest),
        _ => Err(Err::Failure(AccessorParserError {
            kind: AccessorParserErrorKind::InvalidEscapeCharacter(ch),
            span: AccessorParserErrorSpan {
                start: rest.get_utf8_column() - 1,
                end: rest.get_utf8_column() - 1 + 1,
            },
        })),
    }
}

fn take_unicode(input: LocatedSpan<&str>) -> PResult<char> {
    let Ok((input, _)) = tag::<_, _, NomError>("{")(input) else {
        let span_start = input.get_utf8_column() - 1;
        return Err(Err::Failure(AccessorParserError{
            kind: AccessorParserErrorKind::InvalidUnicode(InvalidUnicodeError::ExpectedOpeningBracket),
            span: AccessorParserErrorSpan {
                start: span_start,
                end: span_start + 1,
            },
        }));
    };

    let Ok((input, unicode_code_point)) = terminated(take_until::<_, _, NomError>("}"), tag("}"))(input)  else {
        let span_start = input.get_utf8_column() - 1;
        let span_length = input.fragment().chars().count();
        let span_end = span_start + span_length;

        return Err(Err::Failure(AccessorParserError {
            kind: AccessorParserErrorKind::InvalidUnicode(InvalidUnicodeError::MissingClosingBracket),
            span: AccessorParserErrorSpan {
                start: span_start,
                end: span_end,
            },
        }));
    };

    let code_point_error_span = {
        let span_start = unicode_code_point.get_utf8_column() - 1;
        let span_length = unicode_code_point.fragment().chars().count();
        let span_end = span_start + span_length - 1;

        AccessorParserErrorSpan {
            start: span_start,
            end: span_end,
        }
    };

    if unicode_code_point.len() < 2 || unicode_code_point.len() > 8 {
        return Err(Err::Failure(AccessorParserError {
            kind: AccessorParserErrorKind::InvalidUnicode(InvalidUnicodeError::InvalidCodeLength),
            span: code_point_error_span,
        }));
    }

    let Ok(n) = u32::from_str_radix(unicode_code_point.fragment(), 16) else {
        return  Err(Err::Failure(AccessorParserError {
            kind: AccessorParserErrorKind::InvalidUnicode(InvalidUnicodeError::InvalidHexadecimal),
            span: code_point_error_span,
        }));
    };

    let Some(ch) = char::from_u32(n) else {
        return Err(Err::Failure(AccessorParserError{
            kind: AccessorParserErrorKind::InvalidUnicode(InvalidUnicodeError::InvalidCodePoint),
            span: code_point_error_span,
        }));
    };

    Ok((input, ch))
}

fn take_char(input: LocatedSpan<&str>) -> PResult<char> {
    let (rest, ch) = anychar(input)?;
    if RESERVED_TOKEN.contains(&ch) {
        return Err(Err::Failure(AccessorParserError {
            kind: AccessorParserErrorKind::InvalidCharacter(ch),
            span: AccessorParserErrorSpan {
                start: input.get_utf8_column() - 1,
                end: input.get_utf8_column() - 1 + 1,
            },
        }));
    }

    Ok((rest, ch))
}
