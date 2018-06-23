extern crate smallstring;

mod index;
mod parse;
mod range;
mod select;

pub use self::index::*;
pub use self::parse::*;
pub use self::range::*;
pub use self::select::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        let range1 = Range::exclusive(Index::new(1), Index::new(5));
        assert_eq!(Some((1, 4)), range1.bounds(42));
        assert_eq!(Some((1, 4)), range1.bounds(7));
        let range2 = Range::inclusive(Index::new(2), Index::new(-4));
        assert_eq!(Some((2, 5)), range2.bounds(10));
        assert_eq!(None, range2.bounds(3));
    }

    #[test]
    fn index_ranges() {
        let valid_cases = vec![
            (
                Range::exclusive(Index::Forward(0), Index::Forward(3)),
                "0..3",
            ),
            (
                Range::inclusive(Index::Forward(0), Index::Forward(2)),
                "0...2",
            ),
            (
                Range::inclusive(Index::Forward(2), Index::Backward(1)),
                "2...-2",
            ),
            (
                Range::inclusive(Index::Forward(0), Index::Backward(0)),
                "0...-1",
            ),
            (
                Range::exclusive(Index::Backward(2), Index::Backward(0)),
                "-3..-1",
            ),
            (Range::from(Index::Backward(2)), "-3.."),
            (Range::to(Index::Forward(5)), "..5"),
        ];

        for (range, string) in valid_cases {
            assert_eq!(Some(range), parse_index_range(string));
        }

        let invalid_cases = vec!["0..A", "3-3..42"];

        for range in invalid_cases {
            assert_eq!(None, parse_index_range(range))
        }
    }

    #[test]
    fn range_expand() {
        if let Some(_) = parse_range("abc") {
            panic!("parse_range() failed");
        }

        let actual: Vec<String> = parse_range("-3...3").unwrap().collect();
        let expected: Vec<String> = vec![
            "-3".to_owned(),
            "-2".to_owned(),
            "-1".to_owned(),
            "0".to_owned(),
            "1".to_owned(),
            "2".to_owned(),
            "3".to_owned(),
        ];

        assert_eq!(actual, expected);

        let actual: Vec<String> = parse_range("07...12").unwrap().collect();
        let expected: Vec<String> = vec![
            "07".to_owned(),
            "08".to_owned(),
            "09".to_owned(),
            "10".to_owned(),
            "11".to_owned(),
            "12".to_owned(),
        ];

        assert_eq!(actual, expected);

        let actual: Vec<String> = parse_range("-3...10").unwrap().collect();
        let expected: Vec<String> = vec![
            "-3".to_owned(),
            "-2".to_owned(),
            "-1".to_owned(),
            "0".to_owned(),
            "1".to_owned(),
            "2".to_owned(),
            "3".to_owned(),
            "4".to_owned(),
            "5".to_owned(),
            "6".to_owned(),
            "7".to_owned(),
            "8".to_owned(),
            "9".to_owned(),
            "10".to_owned(),
        ];

        assert_eq!(actual, expected);

        let actual: Vec<String> = parse_range("3...-3").unwrap().collect();
        let expected: Vec<String> = vec![
            "3".to_owned(),
            "2".to_owned(),
            "1".to_owned(),
            "0".to_owned(),
            "-1".to_owned(),
            "-2".to_owned(),
            "-3".to_owned(),
        ];

        assert_eq!(actual, expected);

        let actual: Vec<String> = parse_range("03...-3").unwrap().collect();
        let expected: Vec<String> = vec![
            "03".to_owned(),
            "02".to_owned(),
            "01".to_owned(),
            "00".to_owned(),
            "-1".to_owned(),
            "-2".to_owned(),
            "-3".to_owned(),
        ];

        assert_eq!(actual, expected);

        let actual: Vec<String> = parse_range("3...-03").unwrap().collect();
        let expected: Vec<String> = vec![
            "003".to_owned(),
            "002".to_owned(),
            "001".to_owned(),
            "000".to_owned(),
            "-01".to_owned(),
            "-02".to_owned(),
            "-03".to_owned(),
        ];

        assert_eq!(actual, expected);

        let actual: Vec<String> = parse_range("a...c").unwrap().collect();
        let expected: Vec<String> = vec!["a".to_owned(), "b".to_owned(), "c".to_owned()];

        assert_eq!(actual, expected);

        let actual: Vec<String> = parse_range("c...a").unwrap().collect();
        let expected: Vec<String> = vec!["c".to_owned(), "b".to_owned(), "a".to_owned()];

        assert_eq!(actual, expected);

        let actual: Vec<String> = parse_range("A...C").unwrap().collect();
        let expected: Vec<String> = vec!["A".to_owned(), "B".to_owned(), "C".to_owned()];

        assert_eq!(actual, expected);

        let actual: Vec<String> = parse_range("C...A").unwrap().collect();
        let expected: Vec<String> = vec!["C".to_owned(), "B".to_owned(), "A".to_owned()];

        assert_eq!(actual, expected);

        let actual: Vec<String> = parse_range("C..A").unwrap().collect();
        let expected: Vec<String> = vec!["C".to_owned(), "B".to_owned()];
        assert_eq!(actual, expected);

        let actual: Vec<String> = parse_range("c..a").unwrap().collect();
        let expected: Vec<String> = vec!["c".to_owned(), "b".to_owned()];
        assert_eq!(actual, expected);

        let actual: Vec<String> = parse_range("-3..4").unwrap().collect();
        let expected: Vec<String> = vec![
            "-3".to_owned(),
            "-2".to_owned(),
            "-1".to_owned(),
            "0".to_owned(),
            "1".to_owned(),
            "2".to_owned(),
            "3".to_owned(),
        ];

        assert_eq!(actual, expected);

        let actual: Vec<String> = parse_range("3..-4").unwrap().collect();
        let expected: Vec<String> = vec![
            "3".to_owned(),
            "2".to_owned(),
            "1".to_owned(),
            "0".to_owned(),
            "-1".to_owned(),
            "-2".to_owned(),
            "-3".to_owned(),
        ];

        assert_eq!(actual, expected);

        let actual: Vec<String> = parse_range("-3...0").unwrap().collect();
        let expected: Vec<String> = vec!["-3".into(), "-2".into(), "-1".into(), "0".into()];
        assert_eq!(actual, expected);

        let actual: Vec<String> = parse_range("-3..0").unwrap().collect();
        let expected: Vec<String> = vec!["-3".into(), "-2".into(), "-1".into()];
        assert_eq!(actual, expected);
    }
}
