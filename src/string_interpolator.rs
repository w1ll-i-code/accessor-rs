use nom_locate::LocatedSpan;

use crate::{
    error::AccessorParserError,
    parser::{take_spanned_accessor, take_string_with_escape_until},
    Accessor, SpannedAccessor,
};

#[derive(Debug)]
pub struct SpannedStringInterpolator {
    segments: Vec<SpannedInterpolatorSegment>,
    postfix: Box<str>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct StringInterpolator {
    segments: Box<[InterpolatorSegment]>,
    postfix: Box<str>,
}

impl From<SpannedStringInterpolator> for StringInterpolator {
    fn from(value: SpannedStringInterpolator) -> Self {
        StringInterpolator {
            segments: value.segments.into_iter().map(Into::into).collect(),
            postfix: value.postfix,
        }
    }
}

#[derive(Debug)]
pub struct SpannedInterpolatorSegment {
    prefix: Box<str>,
    accessor: SpannedAccessor,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct InterpolatorSegment {
    prefix: Box<str>,
    accessor: Accessor,
}

impl From<SpannedInterpolatorSegment> for InterpolatorSegment {
    fn from(value: SpannedInterpolatorSegment) -> Self {
        InterpolatorSegment {
            accessor: value.accessor.into(),
            prefix: value.prefix,
        }
    }
}

pub fn take_spanned_string_interpolator(
    input: LocatedSpan<&str>,
) -> Result<SpannedStringInterpolator, nom::Err<AccessorParserError>> {
    let mut segments = vec![];
    let mut input = input;

    loop {
        let (rest, prefix) = take_string_with_escape_until(|c| c == '$', &['$'])(input)?;
        if rest.is_empty() {
            return Ok(SpannedStringInterpolator {
                segments: segments.into(),
                postfix: prefix.into(),
            });
        }

        let (rest, accessor) = take_spanned_accessor(rest)?;
        segments.push(SpannedInterpolatorSegment {
            prefix: prefix.into(),
            accessor,
        });
        input = rest;
    }
}

#[cfg(test)]
mod test {
    use crate::{
        string_interpolator::take_spanned_string_interpolator, AccessorKey, AccessorParserSpan,
        SpannedAccessor, SpannedAccessorKey,
    };

    use super::{SpannedInterpolatorSegment, SpannedStringInterpolator};

    #[test]
    fn should_take_string_interpolation_with_postfix() {
        let interpolator = take_spanned_string_interpolator("${item} -".into()).unwrap();
        let segments = match interpolator {
            SpannedStringInterpolator { segments, postfix } if postfix.as_ref() == " -" => segments,
            err => unreachable!("{:?}", err),
        };

        let keys = match segments.as_slice() {
            [SpannedInterpolatorSegment {
                prefix,
                accessor:
                    SpannedAccessor {
                        keys,
                        span: AccessorParserSpan { start: 0, end: 7 },
                    },
            }] if prefix.as_ref() == "" => keys,
            err => unreachable!("{:?}", err),
        };

        match keys.as_ref() {
            [SpannedAccessorKey {
                key: AccessorKey::String(key),
                span: AccessorParserSpan { start: 2, end: 6 },
            }] if key.as_ref() == "item" => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_take_string_interpolation_with_prefix() {
        let interpolator = take_spanned_string_interpolator("- ${item}".into()).unwrap();
        let segments = match interpolator {
            SpannedStringInterpolator { segments, postfix } if postfix.as_ref() == "" => segments,
            err => unreachable!("{:?}", err),
        };

        let keys = match segments.as_slice() {
            [SpannedInterpolatorSegment {
                prefix,
                accessor:
                    SpannedAccessor {
                        keys,
                        span: AccessorParserSpan { start: 2, end: 9 },
                    },
            }] if prefix.as_ref() == "- " => keys,
            err => unreachable!("{:?}", err),
        };

        match keys.as_ref() {
            [SpannedAccessorKey {
                key: AccessorKey::String(key),
                span: AccessorParserSpan { start: 4, end: 8 },
            }] if key.as_ref() == "item" => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_take_string_interpolation_with_pre_and_postfix() {
        let interpolator = take_spanned_string_interpolator("- ${item} -".into()).unwrap();
        let segments = match interpolator {
            SpannedStringInterpolator { segments, postfix } if postfix.as_ref() == " -" => segments,
            err => unreachable!("{:?}", err),
        };

        let keys = match segments.as_slice() {
            [SpannedInterpolatorSegment {
                prefix,
                accessor:
                    SpannedAccessor {
                        keys,
                        span: AccessorParserSpan { start: 2, end: 9 },
                    },
            }] if prefix.as_ref() == "- " => keys,
            err => unreachable!("{:?}", err),
        };

        match keys.as_ref() {
            [SpannedAccessorKey {
                key: AccessorKey::String(key),
                span: AccessorParserSpan { start: 4, end: 8 },
            }] if key.as_ref() == "item" => {}
            err => unreachable!("{:?}", err),
        }
    }

    #[test]
    fn should_take_string_interpolation_with_multiple_accessor() {
        let interpolator =
            take_spanned_string_interpolator("${event.created_ms} - ${item}".into()).unwrap();
        let segments = match interpolator {
            SpannedStringInterpolator { segments, postfix } if postfix.as_ref() == "" => segments,
            err => unreachable!("{:?}", err),
        };

        let (keys1, keys2) = match segments.as_slice() {
            [SpannedInterpolatorSegment {
                prefix: prefix1,
                accessor:
                    SpannedAccessor {
                        keys: keys1,
                        span: AccessorParserSpan { start: 0, end: 19 },
                    },
            }, SpannedInterpolatorSegment {
                prefix: prefix2,
                accessor:
                    SpannedAccessor {
                        keys: keys2,
                        span: AccessorParserSpan { start: 22, end: 29 },
                    },
            }] if prefix1.as_ref() == "" && prefix2.as_ref() == " - " => (keys1, keys2),
            err => unreachable!("{:?}", err),
        };

        match keys1.as_ref() {
            [SpannedAccessorKey {
                key: AccessorKey::String(key1),
                span: AccessorParserSpan { start: 2, end: 7 },
            }, SpannedAccessorKey {
                key: AccessorKey::String(key2),
                span: AccessorParserSpan { start: 7, end: 18 },
            }] if key1.as_ref() == "event" && key2.as_ref() == "created_ms" => {}
            err => unreachable!("{:?}", err),
        }

        match keys2.as_ref() {
            [SpannedAccessorKey {
                key: AccessorKey::String(key1),
                span: AccessorParserSpan { start: 24, end: 28 },
            }] if key1.as_ref() == "item" => {}
            err => unreachable!("{:?}", err),
        }
    }
}
