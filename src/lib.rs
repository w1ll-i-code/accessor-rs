pub mod error;
pub mod parser;

#[derive(Clone, Debug)]
pub struct SpannedAccessor {
    keys: Box<[SpannedAccessorKey]>,
    span: AccessorParserSpan,
}

impl SpannedAccessor {
    pub fn keys(&self) -> &[SpannedAccessorKey] {
        &self.keys
    }

    pub fn span(&self) -> AccessorParserSpan {
        self.span
    }
}

#[derive(Clone, Debug)]
pub struct Accessor {
    keys: Box<[AccessorKey]>,
}

impl Accessor {
    pub fn keys(&self) -> &[AccessorKey] {
        &self.keys
    }
}

impl From<SpannedAccessor> for Accessor {
    fn from(value: SpannedAccessor) -> Self {
        Accessor {
            keys: value.keys.into_vec().into_iter().map(|a| a.key).collect(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SpannedAccessorKey {
    key: AccessorKey,
    span: AccessorParserSpan,
}

impl SpannedAccessorKey {
    pub fn key(&self) -> &AccessorKey {
        &self.key
    }

    pub fn span(&self) -> AccessorParserSpan {
        self.span
    }
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

#[derive(Clone, Copy, Debug)]
pub struct AccessorParserSpan {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl AccessorParserSpan {
    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }
}
