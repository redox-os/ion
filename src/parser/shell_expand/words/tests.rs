use super::*;
use types::{Array, Value};

struct Empty;

impl Expander for Empty {}

fn compare(input: &str, expected: Vec<WordToken>) {
    let mut correct = 0;
    for (actual, expected) in WordIterator::new(input, &Empty).zip(expected.iter()) {
        assert_eq!(actual, *expected, "{:?} != {:?}", actual, expected);
        correct += 1;
    }
    assert_eq!(expected.len(), correct);
}

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
fn string_method() {
    let input = "$join(array, 'pattern') $join(array, 'pattern')";
    let expected = vec![
        WordToken::StringMethod(StringMethod {
            method:    "join",
            variable:  "array",
            pattern:   "'pattern'",
            selection: Select::All,
        }),
        WordToken::Whitespace(" "),
        WordToken::StringMethod(StringMethod {
            method:    "join",
            variable:  "array",
            pattern:   "'pattern'",
            selection: Select::All,
        }),
    ];
    compare(input, expected);
}

#[test]
fn escape_with_backslash() {
    let input = "\\$FOO\\$BAR \\$FOO";
    let expected = vec![
        WordToken::Normal("$FOO", false, false),
        WordToken::Normal("$BAR", false, false),
        WordToken::Whitespace(" "),
        WordToken::Normal("$FOO", false, false),
    ];
    compare(input, expected);
}

#[test]
fn array_expressions() {
    let input = "[ one two [three four]] [[one two] three four][0]";
    let first = vec!["one", "two", "[three four]"];
    let second = vec!["[one two]", "three", "four"];
    let expected = vec![
        WordToken::Array(first, Select::All),
        WordToken::Whitespace(" "),
        WordToken::Array(second, Select::Index(Index::new(0))),
    ];
    compare(input, expected);
}

#[test]
fn array_variables() {
    let input = "@array @array[0] @{array[1..]}";
    let expected = vec![
        WordToken::ArrayVariable("array", false, Select::All),
        WordToken::Whitespace(" "),
        WordToken::ArrayVariable("array", false, Select::Index(Index::new(0))),
        WordToken::Whitespace(" "),
        WordToken::ArrayVariable("array", false, Select::Range(Range::from(Index::new(1)))),
    ];
    compare(input, expected);
}

#[test]
fn array_processes() {
    let input = "@(echo one two three) @(echo one two three)[0]";
    let expected = vec![
        WordToken::ArrayProcess("echo one two three", false, Select::All),
        WordToken::Whitespace(" "),
        WordToken::ArrayProcess("echo one two three", false, Select::Index(Index::new(0))),
    ];
    compare(input, expected);
}

#[test]
fn indexes() {
    let input = "@array[0..3] @array[0...3] @array[abc] @array[..3] @array[3..]";
    let expected = vec![
        WordToken::ArrayVariable(
            "array",
            false,
            Select::Range(Range::exclusive(Index::new(0), Index::new(3))),
        ),
        WordToken::Whitespace(" "),
        WordToken::ArrayVariable(
            "array",
            false,
            Select::Range(Range::inclusive(Index::new(0), Index::new(3))),
        ),
        WordToken::Whitespace(" "),
        WordToken::ArrayVariable("array", false, Select::Key(Key::new("abc"))),
        WordToken::Whitespace(" "),
        WordToken::ArrayVariable("array", false, Select::Range(Range::to(Index::new(3)))),
        WordToken::Whitespace(" "),
        WordToken::ArrayVariable("array", false, Select::Range(Range::from(Index::new(3)))),
    ];
    compare(input, expected);
}

#[test]
fn string_keys() {
    let input = "@array['key'] @array[key] @array[]";
    let expected = vec![
        WordToken::ArrayVariable("array", false, Select::Key(Key::new("key"))),
        WordToken::Whitespace(" "),
        WordToken::ArrayVariable("array", false, Select::Key(Key::new("key"))),
        WordToken::Whitespace(" "),
        WordToken::ArrayVariable("array", false, Select::Key(Key::new(""))),
    ];
    compare(input, expected);
}

#[test]
fn nested_processes() {
    let input = "echo $(echo $(echo one)) $(echo one $(echo two) three)";
    let expected = vec![
        WordToken::Normal("echo", false, false),
        WordToken::Whitespace(" "),
        WordToken::Process("echo $(echo one)", false, Select::All),
        WordToken::Whitespace(" "),
        WordToken::Process("echo one $(echo two) three", false, Select::All),
    ];
    compare(input, expected);
}

#[test]
fn words_process_with_quotes() {
    let input = "echo $(git branch | rg '[*]' | awk '{print $2}')";
    let expected = vec![
        WordToken::Normal("echo", false, false),
        WordToken::Whitespace(" "),
        WordToken::Process("git branch | rg '[*]' | awk '{print $2}'", false, Select::All),
    ];
    compare(input, expected);

    let input = "echo $(git branch | rg \"[*]\" | awk '{print $2}')";
    let expected = vec![
        WordToken::Normal("echo", false, false),
        WordToken::Whitespace(" "),
        WordToken::Process("git branch | rg \"[*]\" | awk '{print $2}'", false, Select::All),
    ];
    compare(input, expected);
}

#[test]
fn test_words() {
    let input = "echo $ABC \"${ABC}\" one{$ABC,$ABC} ~ $(echo foo) \"$(seq 1 100)\"";
    let expected = vec![
        WordToken::Normal("echo", false, false),
        WordToken::Whitespace(" "),
        WordToken::Variable("ABC", false, Select::All),
        WordToken::Whitespace(" "),
        WordToken::Variable("ABC", true, Select::All),
        WordToken::Whitespace(" "),
        WordToken::Normal("one", false, false),
        WordToken::Brace(vec!["$ABC", "$ABC"]),
        WordToken::Whitespace(" "),
        WordToken::Normal("~", false, true),
        WordToken::Whitespace(" "),
        WordToken::Process("echo foo", false, Select::All),
        WordToken::Whitespace(" "),
        WordToken::Process("seq 1 100", true, Select::All),
    ];
    compare(input, expected);
}

#[test]
fn test_multiple_escapes() {
    let input = "foo\\(\\) bar\\(\\)";
    let expected = vec![
        WordToken::Normal("foo", false, false),
        WordToken::Normal("(", false, false),
        WordToken::Normal(")", false, false),
        WordToken::Whitespace(" "),
        WordToken::Normal("bar", false, false),
        WordToken::Normal("(", false, false),
        WordToken::Normal(")", false, false),
    ];
    compare(input, expected);
}

#[test]
fn test_arithmetic() {
    let input = "echo $((foo bar baz bing 3 * 2))";
    let expected = vec![
        WordToken::Normal("echo", false, false),
        WordToken::Whitespace(" "),
        WordToken::Arithmetic("foo bar baz bing 3 * 2"),
    ];
    compare(input, expected);
}

#[test]
fn test_globbing() {
    let input = "barbaz* bingcrosb*";
    let expected = vec![
        WordToken::Normal("barbaz*", true, false),
        WordToken::Whitespace(" "),
        WordToken::Normal("bingcrosb*", true, false),
    ];
    compare(input, expected);
}

#[test]
fn test_empty_strings() {
    let input = "rename '' 0 a \"\"";
    let expected = vec![
        WordToken::Normal("rename", false, false),
        WordToken::Whitespace(" "),
        WordToken::Normal("", false, false),
        WordToken::Whitespace(" "),
        WordToken::Normal("0", false, false),
        WordToken::Whitespace(" "),
        WordToken::Normal("a", false, false),
        WordToken::Whitespace(" "),
        WordToken::Normal("", false, false),
    ];
    compare(input, expected);
}

struct WithVars;

impl Expander for WithVars {
    fn variable(&self, var: &str, _: bool) -> Option<Value> {
        match var {
            "pkmn1" => "Pokémon".to_owned().into(),
            "pkmn2" => "Poke\u{0301}mon".to_owned().into(),
            _ => None,
        }
    }
}

#[test]
fn array_methods() {
    let expanders = WithVars;
    let method = ArrayMethod {
        method:    "graphemes",
        variable:  "pkmn1",
        pattern:   Pattern::Whitespace,
        selection: Select::Index(Index::Forward(3)),
    };
    let expected = array!["é"];
    assert_eq!(method.handle_as_array(&expanders), expected);
    let method = ArrayMethod {
        method:    "chars",
        variable:  "pkmn2",
        pattern:   Pattern::Whitespace,
        selection: Select::Index(Index::Forward(3)),
    };
    let expected = array!["e"];
    assert_eq!(method.handle_as_array(&expanders), expected);
    let method = ArrayMethod {
        method:    "bytes",
        variable:  "pkmn2",
        pattern:   Pattern::Whitespace,
        selection: Select::Index(Index::Forward(1)),
    };
    let expected = array!["111"];
    assert_eq!(method.handle_as_array(&expanders), expected);
}
