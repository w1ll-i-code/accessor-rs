mod error;
mod parser;

pub use parser::{take_spanned_accessor, Accessor};

#[cfg(test)]
mod test {
    use crate::parser::take_spanned_accessor;

    #[test]
    fn test() {
        dbg!(take_spanned_accessor("${test[123]}".into()));
    }
}
