mod error;
mod parser;

pub use parser::{take_accessor, Accessor};

#[cfg(test)]
mod test {
    use crate::parser::take_accessor;

    #[test]
    fn test() {
        dbg!(take_accessor("${test[123]}".into()));
    }
}
