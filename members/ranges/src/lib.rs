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
        let range1 = Range::exclusive(Index::new(1), Index::new(5), None);
        assert_eq!(Some((1, 4)), range1.bounds(42));
        assert_eq!(Some((1, 4)), range1.bounds(7));
        let range2 = Range::inclusive(Index::new(2), Index::new(-4), None);
        assert_eq!(Some((2, 5)), range2.bounds(10));
        assert_eq!(None, range2.bounds(3));
    }

    #[test]
    fn index_ranges() {
        let valid_cases = vec![
            (Range::exclusive(Index::Forward(0), Index::Forward(3), None), "0..3"),
            (Range::inclusive(Index::Forward(0), Index::Forward(2), None), "0...2"),
            (Range::inclusive(Index::Forward(0), Index::Forward(4), None), "0..=4"),
            (Range::inclusive(Index::Forward(2), Index::Backward(1), None), "2...-2"),
            (Range::inclusive(Index::Forward(0), Index::Backward(0), None), "0...-1"),
            (Range::exclusive(Index::Backward(2), Index::Backward(0), None), "-3..-1"),
            (Range::from(Index::Backward(2), None), "-3.."),
            (Range::to(Index::Forward(5), None), "..5"),
        ];

        for (range, string) in valid_cases {
            println!("{:?} ---- {:?}", range, string);
            assert_eq!(Some(range), parse_index_range(string));
        }

        let invalid_cases = vec!["0..A", "3-3..42", "0.=3", "0=..3", "0.=.3"];

        for range in invalid_cases {
            println!("{:?}", range);
            assert_eq!(None, parse_index_range(range))
        }
    }

    fn test_range<T: Iterator<Item = i8>>(range: &str, expected: T) {
        let actual: Vec<String> = parse_range(range).unwrap().collect();
        let expected: Vec<_> = expected.map(|i| i.to_string()).collect();

        assert_eq!(actual, expected);
    }

    fn test_fixed_range<T: Iterator<Item = i8>>(range: &str, expected: T, digits: usize) {
        let actual: Vec<String> = parse_range(range).unwrap().collect();
        let expected: Vec<_> = expected.map(|i| format!("{:01$}", i, digits)).collect();

        assert_eq!(actual, expected);
    }

    #[test]
    fn range_expand() {
        if let Some(_) = parse_range::<String>("abc") {
            panic!("parse_range() failed");
        }

        test_range("-3...3", -3..=3);
        test_fixed_range("07...12", 7..=12, 2);
        test_range("-3...10", -3..=10);
        test_range("3...-3", (-3..=3).rev());
        test_fixed_range("03...-3", (-3..=3).rev(), 2);
        test_fixed_range("3...-03", (-3..=3).rev(), 3);
        test_fixed_range("3..=-03", (-3..=3).rev(), 3);
        test_range("-3..4", -3..4);
        test_range("3..-4", (-3..4).rev());
        test_range("-3...0", -3..=0);
        test_range("-3..0", -3..0);

        let actual: Vec<String> = parse_range("a...c").unwrap().collect();
        let expected: Vec<String> = vec!["a".into(), "b".into(), "c".into()];

        assert_eq!(actual, expected);

        let actual: Vec<String> = parse_range("c...a").unwrap().collect();
        let expected: Vec<String> = vec!["c".into(), "b".into(), "a".into()];

        assert_eq!(actual, expected);

        let actual: Vec<String> = parse_range("A...C").unwrap().collect();
        let expected: Vec<String> = vec!["A".into(), "B".into(), "C".into()];

        assert_eq!(actual, expected);

        let actual: Vec<String> = parse_range("C...A").unwrap().collect();
        let also: Vec<String> = parse_range("C..=A").unwrap().collect();
        let expected: Vec<String> = vec!["C".into(), "B".into(), "A".into()];

        assert_eq!(actual, expected);
        assert_eq!(also, expected);

        let actual: Vec<String> = parse_range("C..A").unwrap().collect();
        let expected: Vec<String> = vec!["C".into(), "B".into()];

        assert_eq!(actual, expected);

        let actual: Vec<String> = parse_range("c..a").unwrap().collect();
        let expected: Vec<String> = vec!["c".into(), "b".into()];

        assert_eq!(actual, expected);
    }
}
