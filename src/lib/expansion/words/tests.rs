use super::*;
use crate::expansion::test::DummyExpander;

fn compare(input: &str, expected: &[WordToken<'_>]) {
    let mut correct = 0;
    for (actual, expected) in WordIterator::new(input, true).zip(expected.iter()) {
        assert_eq!(actual, *expected, "{:?} != {:?}", actual, expected);
        correct += 1;
    }
    assert_eq!(expected.len(), correct);
}

#[test]
fn string_method() {
    let input = "$join(array 'pattern') $join(array 'pattern')";
    let expected = &[
        WordToken::StringMethod(StringMethod {
            method:    "join",
            variable:  "array",
            pattern:   "'pattern'",
            selection: None,
        }),
        WordToken::Whitespace(" "),
        WordToken::StringMethod(StringMethod {
            method:    "join",
            variable:  "array",
            pattern:   "'pattern'",
            selection: None,
        }),
    ];
    compare(input, expected);
}

#[test]
fn escape_with_backslash() {
    let input = r#"\$FOO\$BAR \$FOO"#;
    let expected = &[
        WordToken::Normal("$FOO$BAR".into(), false, false),
        WordToken::Whitespace(" "),
        WordToken::Normal("$FOO".into(), false, false),
    ];
    compare(input, expected);
}

#[test]
fn array_expressions() {
    let input = "[ one two [three four]] [[one two] three four][0]";
    let first = vec!["one", "two", "[three four]"];
    let second = vec!["[one two]", "three", "four"];
    let expected = &[
        WordToken::Array(first, None),
        WordToken::Whitespace(" "),
        WordToken::Array(second, Some("0")),
    ];
    compare(input, expected);
}

#[test]
fn array_variables() {
    let input = "@array @array[0] @{array[1..]}";
    let expected = &[
        WordToken::ArrayVariable("array", false, None),
        WordToken::Whitespace(" "),
        WordToken::ArrayVariable("array", false, Some("0")),
        WordToken::Whitespace(" "),
        WordToken::ArrayVariable("array", false, Some("1..")),
    ];
    compare(input, expected);
}

#[test]
fn array_processes() {
    let input = "@(echo one two three) @(echo one two three)[0]";
    let expected = &[
        WordToken::ArrayProcess("echo one two three", false, None),
        WordToken::Whitespace(" "),
        WordToken::ArrayProcess("echo one two three", false, Some("0")),
    ];
    compare(input, expected);
}

#[test]
fn array_process_within_string_process() {
    compare(
        "echo $(let free=[@(free -h)]; echo @free[6]@free[8]/@free[7])",
        &[
            WordToken::Normal("echo".into(), false, false),
            WordToken::Whitespace(" "),
            WordToken::Process("let free=[@(free -h)]; echo @free[6]@free[8]/@free[7]", None),
        ],
    )
}

#[test]
fn indexes() {
    let input = "@array[0..3] @array[0...3] @array[abc] @array[..3] @array[3..]";
    let expected = &[
        WordToken::ArrayVariable("array", false, Some("0..3")),
        WordToken::Whitespace(" "),
        WordToken::ArrayVariable("array", false, Some("0...3")),
        WordToken::Whitespace(" "),
        WordToken::ArrayVariable("array", false, Some("abc")),
        WordToken::Whitespace(" "),
        WordToken::ArrayVariable("array", false, Some("..3")),
        WordToken::Whitespace(" "),
        WordToken::ArrayVariable("array", false, Some("3..")),
    ];
    compare(input, expected);
}

#[test]
fn string_keys() {
    let input = "@array['key'] @array[key] @array[]";
    let expected = &[
        WordToken::ArrayVariable("array", false, Some("'key'")),
        WordToken::Whitespace(" "),
        WordToken::ArrayVariable("array", false, Some("key")),
        WordToken::Whitespace(" "),
        WordToken::ArrayVariable("array", false, Some("")),
    ];
    compare(input, expected);
}

#[test]
fn nested_processes() {
    let input = "echo $(echo $(echo one)) $(echo one $(echo two) three)";
    let expected = &[
        WordToken::Normal("echo".into(), false, false),
        WordToken::Whitespace(" "),
        WordToken::Process("echo $(echo one)", None),
        WordToken::Whitespace(" "),
        WordToken::Process("echo one $(echo two) three", None),
    ];
    compare(input, expected);
}

#[test]
fn words_process_with_quotes() {
    let input = "echo $(git branch | rg '[*]' | awk '{print $2}')";
    let expected = &[
        WordToken::Normal("echo".into(), false, false),
        WordToken::Whitespace(" "),
        WordToken::Process("git branch | rg '[*]' | awk '{print $2}'", None),
    ];
    compare(input, expected);

    let input = "echo $(git branch | rg \"[*]\" | awk '{print $2}')";
    let expected = &[
        WordToken::Normal("echo".into(), false, false),
        WordToken::Whitespace(" "),
        WordToken::Process("git branch | rg \"[*]\" | awk '{print $2}'", None),
    ];
    compare(input, expected);
}

#[test]
fn test_words() {
    let input = "echo $ABC \"${ABC}\" one{$ABC,$ABC} ~ $(echo foo) \"$(seq 1 100)\"";
    let expected = &[
        WordToken::Normal("echo".into(), false, false),
        WordToken::Whitespace(" "),
        WordToken::Variable("ABC", None),
        WordToken::Whitespace(" "),
        WordToken::Variable("ABC", None),
        WordToken::Whitespace(" "),
        WordToken::Normal("one".into(), false, false),
        WordToken::Brace(vec!["$ABC", "$ABC"]),
        WordToken::Whitespace(" "),
        WordToken::Normal("~".into(), false, true),
        WordToken::Whitespace(" "),
        WordToken::Process("echo foo", None),
        WordToken::Whitespace(" "),
        WordToken::Process("seq 1 100", None),
    ];
    compare(input, expected);
}

#[test]
fn test_multiple_escapes() {
    let input = "foo\\(\\) bar\\(\\)";
    let expected = &[
        WordToken::Normal("foo()".into(), false, false),
        WordToken::Whitespace(" "),
        WordToken::Normal("bar()".into(), false, false),
    ];
    compare(input, expected);
}

#[test]
fn test_arithmetic() {
    let input = "echo $((foo bar baz bing 3 * 2))";
    let expected = &[
        WordToken::Normal("echo".into(), false, false),
        WordToken::Whitespace(" "),
        WordToken::Arithmetic("foo bar baz bing 3 * 2"),
    ];
    compare(input, expected);
}

#[test]
fn test_globbing() {
    let input = "barbaz* bingcrosb*";
    let expected = &[
        WordToken::Normal("barbaz*".into(), true, false),
        WordToken::Whitespace(" "),
        WordToken::Normal("bingcrosb*".into(), true, false),
    ];
    compare(input, expected);
}

#[test]
fn test_empty_strings() {
    let input = "rename '' 0 a \"\"";
    let expected = &[
        WordToken::Normal("rename".into(), false, false),
        WordToken::Whitespace(" "),
        WordToken::Normal("".into(), false, false),
        WordToken::Whitespace(" "),
        WordToken::Normal("0".into(), false, false),
        WordToken::Whitespace(" "),
        WordToken::Normal("a".into(), false, false),
        WordToken::Whitespace(" "),
        WordToken::Normal("".into(), false, false),
    ];
    compare(input, expected);
}

#[test]
fn test_braces() {
    let input = "echo {c[a,b],d}";
    let expected = &[
        WordToken::Normal("echo".into(), false, false),
        WordToken::Whitespace(" "),
        WordToken::Brace(vec!["c[a,b]", "d"]),
    ];
    compare(input, expected);
}

#[test]
fn array_methods() {
    let method = ArrayMethod::new("graphemes", "pkmn1", Pattern::Whitespace, Some("3"));
    let expected = args!["é"];
    assert_eq!(method.handle_as_array(&DummyExpander).unwrap(), expected);
    let method = ArrayMethod::new("chars", "pkmn2", Pattern::Whitespace, Some("3"));
    let expected = args!["e"];
    assert_eq!(method.handle_as_array(&DummyExpander).unwrap(), expected);
    let method = ArrayMethod::new("bytes", "pkmn2", Pattern::Whitespace, Some("1"));
    let expected = args!["111"];
    assert_eq!(method.handle_as_array(&DummyExpander).unwrap(), expected);
}
