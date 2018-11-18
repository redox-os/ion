extern crate small;

mod index;
mod parse;
mod range;
mod select;

pub use self::{index::*, parse::*, range::*, select::*};

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
                Range::inclusive(Index::Forward(0), Index::Forward(4)),
                "0..=4",
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

        let invalid_cases = vec!["0..A", "3-3..42", "0.=3", "0=..3", "0.=.3"];

        for range in invalid_cases {
            assert_eq!(None, parse_index_range(range))
        }
    }

    #[test]
    fn range_expand() {
        if let Some(_) = parse_range("abc") {
            panic!("parse_range() failed");
        }

        let actual: Vec<small::String> = parse_range("-3...3").unwrap().collect();
        let expected: Vec<small::String> = vec![
            "-3".into(),
            "-2".into(),
            "-1".into(),
            "0".into(),
            "1".into(),
            "2".into(),
            "3".into(),
        ];

        assert_eq!(actual, expected);

        let actual: Vec<small::String> = parse_range("07...12").unwrap().collect();
        let expected: Vec<small::String> = vec![
            "07".into(),
            "08".into(),
            "09".into(),
            "10".into(),
            "11".into(),
            "12".into(),
        ];

        assert_eq!(actual, expected);

        let actual: Vec<small::String> = parse_range("-3...10").unwrap().collect();
        let expected: Vec<small::String> = vec![
            "-3".into(),
            "-2".into(),
            "-1".into(),
            "0".into(),
            "1".into(),
            "2".into(),
            "3".into(),
            "4".into(),
            "5".into(),
            "6".into(),
            "7".into(),
            "8".into(),
            "9".into(),
            "10".into(),
        ];

        assert_eq!(actual, expected);

        let actual: Vec<small::String> = parse_range("3...-3").unwrap().collect();
        let expected: Vec<small::String> = vec![
            "3".into(),
            "2".into(),
            "1".into(),
            "0".into(),
            "-1".into(),
            "-2".into(),
            "-3".into(),
        ];

        assert_eq!(actual, expected);

        let actual: Vec<small::String> = parse_range("03...-3").unwrap().collect();
        let expected: Vec<small::String> = vec![
            "03".into(),
            "02".into(),
            "01".into(),
            "00".into(),
            "-1".into(),
            "-2".into(),
            "-3".into(),
        ];

        assert_eq!(actual, expected);

        let actual: Vec<small::String> = parse_range("3...-03").unwrap().collect();
        let also: Vec<small::String> = parse_range("3..=-03").unwrap().collect();
        let expected: Vec<small::String> = vec![
            "003".into(),
            "002".into(),
            "001".into(),
            "000".into(),
            "-01".into(),
            "-02".into(),
            "-03".into(),
        ];

        assert_eq!(actual, expected);
        assert_eq!(also, expected);

        let actual: Vec<small::String> = parse_range("a...c").unwrap().collect();
        let expected: Vec<small::String> = vec!["a".into(), "b".into(), "c".into()];

        assert_eq!(actual, expected);

        let actual: Vec<small::String> = parse_range("c...a").unwrap().collect();
        let expected: Vec<small::String> = vec!["c".into(), "b".into(), "a".into()];

        assert_eq!(actual, expected);

        let actual: Vec<small::String> = parse_range("A...C").unwrap().collect();
        let expected: Vec<small::String> = vec!["A".into(), "B".into(), "C".into()];

        assert_eq!(actual, expected);

        let actual: Vec<small::String> = parse_range("C...A").unwrap().collect();
        let also: Vec<small::String> = parse_range("C..=A").unwrap().collect();
        let expected: Vec<small::String> = vec!["C".into(), "B".into(), "A".into()];

        assert_eq!(actual, expected);
        assert_eq!(also, expected);

        let actual: Vec<small::String> = parse_range("C..A").unwrap().collect();
        let expected: Vec<small::String> = vec!["C".into(), "B".into()];

        assert_eq!(actual, expected);

        let actual: Vec<small::String> = parse_range("c..a").unwrap().collect();
        let expected: Vec<small::String> = vec!["c".into(), "b".into()];

        assert_eq!(actual, expected);

        let actual: Vec<small::String> = parse_range("-3..4").unwrap().collect();
        let expected: Vec<small::String> = vec![
            "-3".into(),
            "-2".into(),
            "-1".into(),
            "0".into(),
            "1".into(),
            "2".into(),
            "3".into(),
        ];

        assert_eq!(actual, expected);

        let actual: Vec<small::String> = parse_range("3..-4").unwrap().collect();
        let expected: Vec<small::String> = vec![
            "3".into(),
            "2".into(),
            "1".into(),
            "0".into(),
            "-1".into(),
            "-2".into(),
            "-3".into(),
        ];

        assert_eq!(actual, expected);

        let actual: Vec<small::String> = parse_range("-3...0").unwrap().collect();
        let expected: Vec<small::String> = vec!["-3".into(), "-2".into(), "-1".into(), "0".into()];
        assert_eq!(actual, expected);

        let actual: Vec<small::String> = parse_range("-3..0").unwrap().collect();
        let expected: Vec<small::String> = vec!["-3".into(), "-2".into(), "-1".into()];
        assert_eq!(actual, expected);
    }
}
