use std::collections::HashMap;

use crate::{
    error::{AccessorValidationError, AccessorValidationErrorKind},
    string_interpolator::SpannedStringInterpolator,
    AccessorKey, AccessorParserSpan, SpannedAccessor, SpannedAccessorKey,
};

// ToDo: Wrap node into a function and revert the dependencies for better erroros?
pub enum PathNode {
    Node { children: HashMap<String, PathNode> },
    Root,
    ObjectRoot,
    KnownField,
}

impl PathNode {
    pub fn validate_accessor(
        &self,
        accessor: &SpannedAccessor,
    ) -> Result<(), AccessorValidationError> {
        self.validate(accessor, false)
    }

    pub fn validate_interpolator(
        &self,
        interpolator: &SpannedStringInterpolator,
    ) -> Result<(), Vec<AccessorValidationError>> {
        let mut errors = vec![];

        for segment in &interpolator.segments {
            let Err(err) = self.validate(&segment.accessor, true) else {
                continue;
            };

            errors.push(err)
        }

        if errors.is_empty() {
            return Ok(());
        }

        Err(errors)
    }

    fn validate(
        &self,
        accessor: &SpannedAccessor,
        is_interpolator: bool,
    ) -> Result<(), AccessorValidationError> {
        let SpannedAccessor { keys, span } = accessor;
        path_contains(self, span, keys, is_interpolator)
    }
}

fn path_contains(
    node: &PathNode,
    accessor_span: &AccessorParserSpan,
    remaining_keys: &[SpannedAccessorKey],
    is_interpolator: bool,
) -> Result<(), AccessorValidationError> {
    match node {
        PathNode::Root => Ok(()),
        PathNode::ObjectRoot if remaining_keys.is_empty() && is_interpolator => {
            Err(AccessorValidationError {
                kind: AccessorValidationErrorKind::NotStringRepresentable,
                span: *accessor_span,
            })
        }
        PathNode::ObjectRoot => match remaining_keys {
            []
            | [SpannedAccessorKey {
                key: AccessorKey::String(_),
                ..
            }, ..] => Ok(()),
            [SpannedAccessorKey { span, .. }, ..] => Err(AccessorValidationError {
                kind: AccessorValidationErrorKind::NumericIndexInMap,
                span: *span,
            }),
        },
        PathNode::KnownField => match remaining_keys {
            [] => Ok(()),
            [SpannedAccessorKey { span, .. }, ..] => Err(AccessorValidationError {
                kind: AccessorValidationErrorKind::NotIndexable,
                span: *span,
            }),
        },
        PathNode::Node { children } => match remaining_keys {
            [] if !is_interpolator => Ok(()),
            [] => Err(AccessorValidationError {
                kind: AccessorValidationErrorKind::NotStringRepresentable,
                span: *accessor_span,
            }),
            [SpannedAccessorKey {
                key: AccessorKey::Numeric(_),
                span,
            }, ..] => Err(AccessorValidationError {
                kind: AccessorValidationErrorKind::NumericIndexInMap,
                span: *span,
            }),
            [SpannedAccessorKey {
                key: AccessorKey::String(key),
                span,
            }, remaining_keys @ ..] => match children.get(key.as_ref()) {
                Some(node) => path_contains(node, accessor_span, remaining_keys, is_interpolator),
                None => {
                    let mut keys: Vec<_> = children
                        .keys()
                        .cloned()
                        .map(|s| (edit_distance(&s, key.as_ref()), s))
                        .collect();
                    keys.sort();
                    let keys = keys.into_iter().map(|(_edit_distance, key)| key).collect();

                    Err(AccessorValidationError {
                        kind: AccessorValidationErrorKind::UnknownKey {
                            possible_keys: keys,
                        },
                        span: *span,
                    })
                }
            },
        },
    }
}

// see wikipedia: https://en.wikipedia.org/wiki/Levenshtein_distance
fn edit_distance(s1: &str, s2: &str) -> u32 {
    // store character and distance into a struct to avoid char boundary problems while indexing
    struct DistanceMapEntry {
        distance: u32,
        character: char,
    }

    // Select the shorter string as s1.
    let (s1, s2) = if s1.len() < s2.len() {
        (s1, s2)
    } else {
        (s2, s1)
    };

    // Allocate only memory for string1
    let len = s1.chars().count() + 1;

    // Store the first row in the algorithm (edit distance from empty string)
    // use the null terminator as a place holder. We assume no one uses that in json keys.
    let mut table_buffer = Vec::with_capacity(len);
    table_buffer.push(DistanceMapEntry {
        distance: 0,
        character: '\0',
    });

    for (idx, ch1) in s1.chars().enumerate() {
        table_buffer.push(DistanceMapEntry {
            distance: (idx + 1) as u32,
            character: ch1,
        });
    }

    // Loop through all the characters of the second string
    for (start_distance, ch2) in s2.chars().enumerate() {
        let mut previous_distance = start_distance as u32 + 1;
        // compute the edit distance.
        for idx in 1..len {
            let DistanceMapEntry {
                distance,
                character,
            } = table_buffer[idx];
            let mut this_distance = previous_distance
                .min(distance)
                .min(table_buffer[idx - 1].distance);
            if ch2 != character {
                this_distance += 1;
            }
            (table_buffer[idx - 1].distance, previous_distance) =
                (previous_distance, this_distance);
        }

        // flush the last value out.
        table_buffer[len - 1].distance = previous_distance;
    }

    table_buffer[len - 1].distance
}

#[cfg(test)]
mod test {
    use maplit::hashmap;

    use super::{edit_distance, PathNode};
    use crate::{
        parser::take_spanned_accessor, string_interpolator::take_spanned_string_interpolator,
    };

    fn test_path_tree() -> PathNode {
        PathNode::Node {
            children: hashmap! {
                "event".to_owned() => PathNode::Node { children: hashmap! {
                    "created_ms".to_owned() => PathNode::KnownField,
                    "metadata".to_owned() => PathNode::ObjectRoot,
                    "payload".to_owned() => PathNode::ObjectRoot,
                }},
                "item".to_owned() => PathNode::Root,
                "_variables".to_owned() => PathNode::Node { children: hashmap! {
                    "target1".to_owned() => PathNode::Root,
                    "target2".to_owned() => PathNode::Root,
                    "target3".to_owned() => PathNode::KnownField,
                }},
            },
        }
    }

    #[test]
    fn should_compute_edit_distance() {
        // see wikipedia: https://en.wikipedia.org/wiki/Levenshtein_distance#Iterative_with_full_matrix
        assert_eq!(3, edit_distance("saturday", "sunday"));
        assert_eq!(3, edit_distance("kitten", "sitting"));
        assert_eq!(4, edit_distance("levenshtein", "meilenstein"));
    }

    #[test]
    fn should_validate_accessor_correctly() {
        let valid_mappings = test_path_tree();

        let (_, accessor) = take_spanned_accessor("${event.created_ms}".into()).unwrap();
        valid_mappings.validate_accessor(&accessor).unwrap();

        let (_, accessor) = take_spanned_accessor("${item}".into()).unwrap();
        valid_mappings.validate_accessor(&accessor).unwrap();

        let (_, accessor) = take_spanned_accessor("${_variables.target1.pippo}".into()).unwrap();
        valid_mappings.validate_accessor(&accessor).unwrap();
    }

    #[test]
    fn should_validate_interpolator_correctly() {
        let valid_mappings = test_path_tree();

        let interpolator =
            take_spanned_string_interpolator("${event.created_ms} - ${item}".into()).unwrap();
        valid_mappings.validate_interpolator(&interpolator).unwrap();

        let interpolator =
            take_spanned_string_interpolator("${item.pippo} - _variables.target1[1234]".into())
                .unwrap();
        valid_mappings.validate_interpolator(&interpolator).unwrap();

        let interpolator =
            take_spanned_string_interpolator("${_variables.target1.pippo}".into()).unwrap();
        valid_mappings.validate_interpolator(&interpolator).unwrap();
    }
}
