use nom::{
    branch::alt,
    bytes::complete::{tag, take_until},
    character::complete::anychar,
    error::Error,
    sequence::terminated,
    Err,
};
use nom_locate::LocatedSpan;

use crate::error::{
    AccessorParserError, AccessorParserErrorKind, AccessorParserSpan, InvalidUnicodeError,
};

const RESERVED_TOKEN: &[char] = &['\\', '{', '}', '[', ']', '.', '$'];

type PResult<'input, Output> = Result<(LocatedSpan<&'input str>, Output), Err<AccessorParserError>>;
type NomError<'input> = Error<LocatedSpan<&'input str>>;

#[derive(Clone, Debug)]
pub struct SpannedAccessor {
    keys: Box<[SpannedAccessorKey]>,
    span: AccessorParserSpan,
}

#[derive(Clone, Debug)]
pub struct Accessor {
    keys: Box<[AccessorKey]>,
}

#[derive(Clone, Debug)]
pub struct SpannedAccessorKey {
    key: AccessorKey,
    span: AccessorParserSpan,
}

#[derive(Clone, Debug)]
pub enum AccessorKey {
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

pub fn take_spanned_accessor<'input>(
    input: LocatedSpan<&'input str>,
) -> PResult<'input, Option<SpannedAccessor>> {
    let fn_input = input;
    let Ok((input, opening)) = tag::<_, _, NomError>("${")(input) else {
        return Ok((input, None));
    };

    let (rest, root) = take_string_until(is_separator)(input)?;
    let root = {
        let span_start = input.get_utf8_column() - 1;
        let span_length_bytes = input.len() - rest.len();
        let span_end = span_start + input[..span_length_bytes].chars().count();
        SpannedAccessorKey {
            key: root.into(),
            span: AccessorParserSpan {
                start: span_start,
                end: span_end,
            },
        }
    };

    let mut keys = vec![root];
    let mut input = rest;

    let error = loop {
        match take_spanned_key(input) {
            Ok((next, key)) => {
                keys.push(key);
                input = next;
            }
            Err(err @ Err::Failure(_)) => return Err(err),
            Err(err) => break err,
        }
    };

    let Ok((input, _)) = tag::<_, _, NomError>("}")(input) else {
        let span_start = opening.get_utf8_column() - 1;
        return Err(Err::Failure(AccessorParserError {
            kind: AccessorParserErrorKind::MissingClosingBracket,
            span: AccessorParserSpan {
                start: span_start,
                end: span_start + 2
            }
        }));
    };

    let span_start = opening.get_utf8_column() - 1;
    let span_length_bytes = input.get_utf8_column() - 1;
    let span_end = span_start + fn_input[..span_length_bytes].chars().count();

    Ok((
        input,
        Some(SpannedAccessor {
            keys: keys.into_boxed_slice(),
            span: AccessorParserSpan {
                start: span_start,
                end: span_end,
            },
        }),
    ))
}

fn take_spanned_key<'input>(
    input: LocatedSpan<&'input str>,
) -> PResult<'input, SpannedAccessorKey> {
    let (rest, key) = take_key(input)?;
    let span_start = input.get_utf8_column() - 1;
    let span_byte_length = input.len() - rest.len();
    let span_end = span_start + input[..span_byte_length].chars().count();
    Ok((
        rest,
        SpannedAccessorKey {
            key,
            span: AccessorParserSpan {
                start: span_start,
                end: span_end,
            },
        },
    ))
}

fn take_key<'input>(input: LocatedSpan<&'input str>) -> PResult<'input, AccessorKey> {
    alt((take_string_key, take_numeric_key))(input)
}

fn take_numeric_key<'input>(input: LocatedSpan<&'input str>) -> PResult<'input, AccessorKey> {
    let Ok((input, opening_bracket)) = tag::<_, _, NomError>("[")(input) else {
        let span_start = input.get_utf8_column() - 1;
        let next_separator = find_next_separator(input);
        let span_end = input.fragment()[..next_separator].chars().count();

        return Err(Err::Error(AccessorParserError {
            kind: AccessorParserErrorKind::InvalidAccessor,
            span: AccessorParserSpan {
                start: span_start,
                end: span_end,
            },
        }));
    };

    let Ok((input, index)) = terminated(take_until("]"), tag::<_, _, NomError>("]"))(input) else {
        let span_start = opening_bracket.get_utf8_column() - 1;
        return Err(Err::Failure(AccessorParserError {
            kind: AccessorParserErrorKind::MissingClosingBracket,
            span: AccessorParserSpan {
                start: span_start,
                end: span_start + 1,
            },
        }));
    };

    let Some(index): Option<usize> = index.parse().ok() else {
        let span_start = index.get_utf8_column() - 1;
        let span_end = span_start + index.chars().count();
        return Err(Err::Failure(AccessorParserError {
            kind: AccessorParserErrorKind::NotANumber,
            span: AccessorParserSpan {
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
            span: AccessorParserSpan {
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

            let (rest, ch) = alt((take_escaped_char, take_char))(input)?;

            input = rest;
            buf.push(ch);
        }
    }
}

fn take_escaped_char<'input>(input: LocatedSpan<&'input str>) -> PResult<'input, char> {
    let (input, first) = tag("\\")(input)?;
    let (rest, ch) = anychar(input)?;
    match ch {
        '\\' | '{' | '}' | '[' | ']' | '.' | '$' => Ok((rest, ch)),
        'n' => Ok((rest, '\n')),
        't' => Ok((rest, '\t')),
        'r' => Ok((rest, '\r')),
        'u' => take_unicode(rest),
        _ => {
            let span_start = first.get_utf8_column() - 1;
            let span_end = rest.get_utf8_column() - 1;
            Err(Err::Failure(AccessorParserError {
                kind: AccessorParserErrorKind::InvalidEscapeCharacter(ch),
                span: AccessorParserSpan {
                    start: span_start,
                    end: span_end,
                },
            }))
        }
    }
}

fn take_unicode(input: LocatedSpan<&str>) -> PResult<char> {
    let Ok((input, _)) = tag::<_, _, NomError>("{")(input) else {
        let span_start = input.get_utf8_column() - 1;
        return Err(Err::Failure(AccessorParserError{
            kind: AccessorParserErrorKind::InvalidUnicode(InvalidUnicodeError::MissingOpeningBracket),
            span: AccessorParserSpan {
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
            span: AccessorParserSpan {
                start: span_start,
                end: span_end,
            },
        }));
    };

    let code_point_error_span = {
        let span_start = unicode_code_point.get_utf8_column() - 1;
        let span_length = unicode_code_point.fragment().chars().count();
        let span_end = span_start + span_length;

        AccessorParserSpan {
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
            span: AccessorParserSpan {
                start: input.get_utf8_column() - 1,
                end: input.get_utf8_column() - 1 + 1,
            },
        }));
    }

    Ok((rest, ch))
}

#[cfg(test)]
mod tests {
    use nom::multi::many0;

    use crate::{
        error::{
            AccessorParserError, AccessorParserErrorKind, AccessorParserSpan, InvalidUnicodeError,
        },
        parser::SpannedAccessorKey,
    };

    use super::{
        take_char, take_escaped_char, take_key, take_numeric_key, take_spanned_accessor,
        take_string_key, take_string_until, take_unicode, AccessorKey,
    };

    #[test]
    fn should_take_single_char() {
        let (rest, ch) = take_char("abcd".into()).unwrap();
        assert_eq!('a', ch);
        assert_eq!("bcd", *rest.fragment());
        assert_eq!(1, rest.get_utf8_column() - 1);
    }

    #[test]
    fn should_fail_to_take_reserved_char() {
        let err = take_char(".bcd".into()).unwrap_err();
        match err {
            nom::Err::Failure(AccessorParserError {
                kind: AccessorParserErrorKind::InvalidCharacter('.'),
                span: AccessorParserSpan { start: 0, end: 1 },
            }) => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_take_multiple_chars() {
        let (input, ch1) = take_char("abcd".into()).unwrap();
        let (rest, ch2) = take_char(input).unwrap();
        assert_eq!('a', ch1);
        assert_eq!('b', ch2);
        assert_eq!("cd", *rest.fragment());
        assert_eq!(2, rest.get_utf8_column() - 1);
    }

    #[test]
    fn should_take_only_first_chars() {
        let (input, ch1) = take_char("a.cd".into()).unwrap();
        let err = take_char(input).unwrap_err();
        assert_eq!('a', ch1);
        match err {
            nom::Err::Failure(AccessorParserError {
                kind: AccessorParserErrorKind::InvalidCharacter('.'),
                span: AccessorParserSpan { start: 1, end: 2 },
            }) => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_parse_correct_unicode() {
        let (rest, ch) = take_unicode("{61}bcd".into()).unwrap();
        assert_eq!('a', ch);
        assert_eq!("bcd", *rest.fragment());
        assert_eq!(4, rest.get_utf8_column() - 1);
    }

    #[test]
    fn should_fail_to_parse_unicode_on_to_short_code() {
        let err = take_unicode("{6}bcd".into()).unwrap_err();
        match err {
            nom::Err::Failure(AccessorParserError {
                kind:
                    AccessorParserErrorKind::InvalidUnicode(InvalidUnicodeError::InvalidCodeLength),
                span: AccessorParserSpan { start: 1, end: 2 },
            }) => {}
            err => unreachable!("{:?}", err),
        }

        let err = take_unicode("{123456789}bcd".into()).unwrap_err();
        match err {
            nom::Err::Failure(AccessorParserError {
                kind:
                    AccessorParserErrorKind::InvalidUnicode(InvalidUnicodeError::InvalidCodeLength),
                span: AccessorParserSpan { start: 1, end: 10 },
            }) => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_fail_to_parse_unicode_on_missing_opening_bracket() {
        let err = take_unicode("6}bcd".into()).unwrap_err();
        match err {
            nom::Err::Failure(AccessorParserError {
                kind:
                    AccessorParserErrorKind::InvalidUnicode(InvalidUnicodeError::MissingOpeningBracket),
                span: AccessorParserSpan { start: 0, end: 1 },
            }) => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_fail_to_parse_unicode_on_missing_closing_bracket() {
        let err = take_unicode("{6bcd".into()).unwrap_err();
        match err {
            nom::Err::Failure(AccessorParserError {
                kind:
                    AccessorParserErrorKind::InvalidUnicode(InvalidUnicodeError::MissingClosingBracket),
                span: AccessorParserSpan { start: 1, end: 5 },
            }) => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_fail_to_parse_unicode_on_invalid_hex() {
        let err = take_unicode("{xx}".into()).unwrap_err();
        match err {
            nom::Err::Failure(AccessorParserError {
                kind:
                    AccessorParserErrorKind::InvalidUnicode(InvalidUnicodeError::InvalidHexadecimal),
                span: AccessorParserSpan { start: 1, end: 3 },
            }) => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_fail_to_parse_unicode_on_invalid_code_point() {
        let err = take_unicode("{10ffffff}".into()).unwrap_err();
        match err {
            nom::Err::Failure(AccessorParserError {
                kind: AccessorParserErrorKind::InvalidUnicode(InvalidUnicodeError::InvalidCodePoint),
                span: AccessorParserSpan { start: 1, end: 9 },
            }) => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_parse_escape_characters() {
        let (rest, ch) = take_escaped_char("\\nopq".into()).unwrap();
        assert_eq!('\n', ch);
        assert_eq!("opq", *rest.fragment());
        assert_eq!(2, rest.get_utf8_column() - 1);

        let (rest, ch) = take_escaped_char("\\.opq".into()).unwrap();
        assert_eq!('.', ch);
        assert_eq!("opq", *rest.fragment());
        assert_eq!(2, rest.get_utf8_column() - 1);

        let (rest, ch) = take_escaped_char("\\u{61}bcd".into()).unwrap();
        assert_eq!('a', ch);
        assert_eq!("bcd", *rest.fragment());
        assert_eq!(6, rest.get_utf8_column() - 1);
    }

    #[test]
    fn should_fail_to_parse_unknown_escape_sequence() {
        let err = take_escaped_char("\\abcd".into()).unwrap_err();
        match err {
            nom::Err::Failure(AccessorParserError {
                kind: AccessorParserErrorKind::InvalidEscapeCharacter('a'),
                span: AccessorParserSpan { start: 0, end: 2 },
            }) => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_take_string() {
        let (rest, string) = take_string_until(|_| false)("\\u{61}bcd\\\\".into()).unwrap();
        assert_eq!("abcd\\", string.as_str());
        assert_eq!("", *rest.fragment());
        assert_eq!(11, rest.get_utf8_column() - 1);
    }

    #[test]
    fn should_take_string_until() {
        let (rest, string) = take_string_until(|c| c == 'c')("\\u{61}bcd\\\\".into()).unwrap();
        assert_eq!("ab", string.as_str());
        assert_eq!("cd\\\\", *rest.fragment());
        assert_eq!(7, rest.get_utf8_column() - 1);
    }

    #[test]
    fn should_fail_to_take_string_on_reserved_character() {
        let err = take_string_until(|_| false)("ab.c".into()).unwrap_err();
        match err {
            nom::Err::Failure(AccessorParserError {
                kind: AccessorParserErrorKind::InvalidCharacter('.'),
                span: AccessorParserSpan { start: 2, end: 3 },
            }) => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_fail_to_take_string_on_invalid_escape_char() {
        let err = take_string_until(|_| false)("ab\\c".into()).unwrap_err();
        match err {
            nom::Err::Failure(AccessorParserError {
                kind: AccessorParserErrorKind::InvalidEscapeCharacter('c'),
                span: AccessorParserSpan { start: 2, end: 4 },
            }) => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_take_string_key() {
        let (rest, key) = take_string_key(".key".into()).unwrap();
        assert_eq!("", *rest.fragment());
        assert_eq!(4, rest.get_utf8_column() - 1);
        match key {
            AccessorKey::String(s) if s.as_ref() == "key" => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_take_first_string_key() {
        let (rest, key) = take_string_key(".key.key".into()).unwrap();
        assert_eq!(".key", *rest.fragment());
        assert_eq!(4, rest.get_utf8_column() - 1);
        match key {
            AccessorKey::String(s) if s.as_ref() == "key" => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_take_first_key() {
        let (rest, key) = take_string_key(".key[1234]".into()).unwrap();
        assert_eq!("[1234]", *rest.fragment());
        assert_eq!(4, rest.get_utf8_column() - 1);
        match key {
            AccessorKey::String(s) if s.as_ref() == "key" => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_take_last_string_key() {
        let (rest, key) = take_string_key(".key}".into()).unwrap();
        assert_eq!("}", *rest.fragment());
        assert_eq!(4, rest.get_utf8_column() - 1);
        match key {
            AccessorKey::String(s) if s.as_ref() == "key" => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_fail_to_take_string_key_without_prefix() {
        let err = take_string_key("key".into()).unwrap_err();
        match err {
            nom::Err::Error(AccessorParserError {
                kind: AccessorParserErrorKind::InvalidAccessor,
                span: AccessorParserSpan { start: 0, end: 3 },
            }) => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_fail_to_take_string_key_without_prefix_and_trailing_key() {
        let err = take_string_key("key.key".into()).unwrap_err();
        match err {
            nom::Err::Error(AccessorParserError {
                kind: AccessorParserErrorKind::InvalidAccessor,
                span: AccessorParserSpan { start: 0, end: 3 },
            }) => {}
            err => unreachable!("{:?}", err),
        }

        let err = take_string_key("key[1234]".into()).unwrap_err();
        match err {
            nom::Err::Error(AccessorParserError {
                kind: AccessorParserErrorKind::InvalidAccessor,
                span: AccessorParserSpan { start: 0, end: 3 },
            }) => {}
            err => unreachable!("{:?}", err),
        }

        let err = take_string_key("key} ---".into()).unwrap_err();
        match err {
            nom::Err::Error(AccessorParserError {
                kind: AccessorParserErrorKind::InvalidAccessor,
                span: AccessorParserSpan { start: 0, end: 3 },
            }) => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_take_numeric_key() {
        let (rest, key) = take_numeric_key("[1234]".into()).unwrap();
        assert_eq!("", *rest.fragment());
        assert_eq!(6, rest.get_utf8_column() - 1);
        match key {
            AccessorKey::Numeric(1234) => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_take_first_numeric_key() {
        let (rest, key) = take_numeric_key("[1234].key".into()).unwrap();
        assert_eq!(".key", *rest.fragment());
        assert_eq!(6, rest.get_utf8_column() - 1);
        match key {
            AccessorKey::Numeric(1234) => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_fail_to_take_numeric_key_on_missing_opening_bracket() {
        let err = take_numeric_key("1234]".into()).unwrap_err();
        match err {
            nom::Err::Error(AccessorParserError {
                kind: AccessorParserErrorKind::InvalidAccessor,
                span: AccessorParserSpan { start: 0, end: 5 },
            }) => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_fail_to_take_numeric_key_on_missing_closing_bracket() {
        let err = take_numeric_key("[1234".into()).unwrap_err();
        match err {
            nom::Err::Failure(AccessorParserError {
                kind: AccessorParserErrorKind::MissingClosingBracket,
                span: AccessorParserSpan { start: 0, end: 1 },
            }) => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_fail_to_take_numeric_key_on_not_a_number() {
        let err = take_numeric_key("[abc]".into()).unwrap_err();
        match err {
            nom::Err::Failure(AccessorParserError {
                kind: AccessorParserErrorKind::NotANumber,
                span: AccessorParserSpan { start: 1, end: 4 },
            }) => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_take_multiple_keys() {
        let (rest, key) = many0(take_key)(".key1[1234].key2".into()).unwrap();
        assert_eq!("", *rest.fragment());
        assert_eq!(16, rest.get_utf8_column() - 1);
        match key.as_slice() {
            [AccessorKey::String(key1), AccessorKey::Numeric(1234), AccessorKey::String(key2)]
                if key1.as_ref() == "key1" && key2.as_ref() == "key2" => {}
            err => unreachable!("{:?}", err),
        }

        let (rest, key) = many0(take_key)(".key\\u{31}[1234].key\\u{32}".into()).unwrap();
        assert_eq!("", *rest.fragment());
        assert_eq!(26, rest.get_utf8_column() - 1);
        match key.as_slice() {
            [AccessorKey::String(key1), AccessorKey::Numeric(1234), AccessorKey::String(key2)]
                if key1.as_ref() == "key1" && key2.as_ref() == "key2" => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_not_return_accessor_without_brackets() {
        let (rest, accessor) = take_spanned_accessor("key1[1234].key2".into()).unwrap();
        assert_eq!("key1[1234].key2", *rest.fragment());
        assert_eq!(0, rest.get_utf8_column() - 1);
        match accessor {
            None => {}
            Some(accessor) => unreachable!("{:?}", accessor),
        }
    }

    #[test]
    fn should_return_accessor_with_root() {
        let (rest, accessor) = take_spanned_accessor("${key}".into()).unwrap();
        assert_eq!("", *rest.fragment());
        assert_eq!(6, rest.get_utf8_column() - 1);
        match accessor {
            Some(accessor) => {
                assert_eq!(1, accessor.keys.len());
                assert_eq!((0, 6), (accessor.span.start, accessor.span.end));
                match accessor.keys.as_ref() {
                    [SpannedAccessorKey {
                        key: AccessorKey::String(key),
                        span: AccessorParserSpan { start: 2, end: 5 },
                    }] if key.as_ref() == "key" => {}
                    err => unreachable!("{:?}", err),
                }
            }
            None => unreachable!(),
        }
    }

    #[test]
    fn should_return_accessor_with_multiple_keys() {
        let (rest, accessor) = take_spanned_accessor("${key1[1234].key2}".into()).unwrap();
        assert_eq!("", *rest.fragment());
        assert_eq!(18, rest.get_utf8_column() - 1);
        match accessor {
            Some(accessor) => {
                assert_eq!((0, 18), (accessor.span.start, accessor.span.end));
                match accessor.keys.as_ref() {
                    [SpannedAccessorKey {
                        key: AccessorKey::String(key1),
                        span: AccessorParserSpan { start: 2, end:6 },
                    }, SpannedAccessorKey {
                        key: AccessorKey::Numeric(1234),
                        span: AccessorParserSpan { start: 6, end: 12 },
                    }, SpannedAccessorKey {
                        key: AccessorKey::String(key2),
                        span: AccessorParserSpan { start: 12, end: 17 },
                    }] if key1.as_ref() == "key1" && key2.as_ref() == "key2" => {}
                    err => unreachable!("{:?}", err),
                }
            }
            None => unreachable!(),
        }
    }

    #[test]
    fn should_fail_to_create_accessor_on_missing_closing_bracket() {
        let err = take_spanned_accessor("${key1[1234].key2".into()).unwrap_err();
        match err {
            nom::Err::Failure(AccessorParserError {
                kind: AccessorParserErrorKind::MissingClosingBracket,
                span: AccessorParserSpan { start: 0, end: 2 },
            }) => {}
            err => unreachable!("{:?}", err),
        }
    }
}
