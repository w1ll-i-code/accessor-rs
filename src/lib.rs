

mod error;
mod parser;

#[cfg(test)]
mod test {
    use crate::parser::take_accessor;

    #[test]
    fn test() {
        dbg!(take_accessor("${test[123]}".into()));
    }
}
