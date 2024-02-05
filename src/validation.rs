use std::collections::HashMap;

use crate::{
    error::{AccessorValidationError, AccessorValidationErrorKind},
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

    pub fn validate_interpolation_accessor(
        &self,
        accessor: &SpannedAccessor,
    ) -> Result<(), AccessorValidationError> {
        self.validate(accessor, true)
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
                None => Err(AccessorValidationError {
                    kind: AccessorValidationErrorKind::UnknownField,
                    span: *span,
                }),
            },
        },
    }
}

#[cfg(test)]
mod test {
    use crate::{
        parser::take_spanned_accessor,
        string_interpolator::{take_spanned_string_interpolator, SpannedStringInterpolator},
        validation::PathNode,
        SpannedAccessor,
    };
    use std::collections::HashMap;

    #[test]
    fn test() {
        let (rest, accessor) = take_spanned_accessor("${event.created_ms.pippo}".into()).unwrap();
        dbg!(&accessor);

        let mut event = HashMap::new();
        event.insert("created_ms".to_owned(), PathNode::KnownField);

        let mut children = HashMap::new();
        children.insert("event".to_owned(), PathNode::Node { children: event });

        let root = PathNode::Node { children };
        let result = root.validate_interpolation_accessor(&accessor);
        dbg!(result);
    }
}
