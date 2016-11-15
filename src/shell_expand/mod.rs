extern crate permutate;

pub mod braces;
mod variables;
mod words;

use self::words::{WordIterator, WordToken};

#[derive(Debug, PartialEq)]
pub enum ExpandErr {
    Brace(braces::BraceErr),
}

/// Performs shell expansions to an input string, efficiently returning the final expanded form.
/// Shells must provide their own batteries for expanding tilde and variable words.
pub fn expand_string<T, V, C>(original: &str, expand_tilde: T, expand_variable: V, expand_command: C) -> Result<String, ExpandErr>
    where T: Fn(&str) -> Option<String>,
          V: Fn(&str) -> Option<String>,
          C: Fn(&str) -> Option<String>,
{
    let mut output = String::with_capacity(original.len() >> 1);
    for word in WordIterator::new(original) {
        match word {
            WordToken::Normal(text) => {
                output.push_str(text);
            },
            WordToken::Tilde(text) => match expand_tilde(text) {
                Some(expanded) => output.push_str(&expanded),
                None           => output.push_str(text),
            },
            WordToken::Variable(text) => {
                variables::expand(&mut output, text, |variable| expand_variable(variable), |command| expand_command(command));
            },
            WordToken::Brace(text, contains_variables) => {
                if contains_variables {
                    let mut temp = String::new();
                    variables::expand(&mut temp, text, |variable| expand_variable(variable), |command| expand_command(command));
                    braces::expand_braces(&mut output, &temp).map_err(ExpandErr::Brace)?;
                } else {
                    braces::expand_braces(&mut output, text).map_err(ExpandErr::Brace)?;
                }
            }
        }
    }
    Ok(output)
}

#[test]
fn expand_long_braces() {
    let line = "The pro{digal,grammer,cessed,totype,cedures,ficiently,ving,spective,jections}";
    let expected = "The prodigal programmer processed prototype procedures proficiently proving prospective projections";
    let expanded = expand_string(line, |_| None, |_| None).unwrap();
    assert_eq!(expected, &expanded);
}

#[test]
fn expand_several_braces() {
    let line = "The {barb,veget}arian eat{ers,ing} appl{esauce,ied} am{ple,ounts} of eff{ort,ectively}";
    let expected = "The barbarian vegetarian eaters eating applesauce applied ample amounts of effort effectively";
    let expanded = expand_string(line, |_| None, |_| None).unwrap();
    assert_eq!(expected, &expanded);
}

#[test]
fn expand_several_variables() {
    let expand_var = |var: &str| match var {
        "FOO" => Some("BAR".to_owned()),
        "X"   => Some("Y".to_owned()),
        _     => None,
    };
    let expanded = expand_string("variables: $FOO $X", |_| None, expand_var).unwrap();
    assert_eq!("variables: BAR Y", &expanded);
}

#[test]
fn expand_variable_braces() {
    let expand_var = |var: &str| if var == "FOO" { Some("BAR".to_owned()) } else { None };
    let expanded = expand_string("FOO$FOO", |_| None, expand_var).unwrap();
    assert_eq!("FOOBAR", &expanded);

    let expand_var = |var: &str| if var == "FOO" { Some("BAR".to_owned()) } else { None };
    let expanded = expand_string(" FOO$FOO ", |_| None, expand_var).unwrap();
    assert_eq!(" FOOBAR ", &expanded);
}

#[test]
fn expand_variables_with_colons() {
    let expand_var = |var: &str| match var {
        "FOO" => Some("FOO".to_owned()),
        "BAR" => Some("BAR".to_owned()),
        _     => None,
    };
    let expanded = expand_string("$FOO:$BAR", |_| None, expand_var).unwrap();
    assert_eq!("FOO:BAR", &expanded);
}

#[test]
fn expand_multiple_variables() {
    let expand_var = |var: &str| match var {
        "A" => Some("test".to_owned()),
        "B" => Some("ing".to_owned()),
        "C" => Some("1 2 3".to_owned()),
        _   => None,
    };
    let expanded = expand_string("${A}${B}...${C}", |_| None, expand_var).unwrap();
    assert_eq!("testing...1 2 3", &expanded);
}

#[test]
fn escape_with_backslash() {
    let expanded = expand_string("\\$FOO", |_| None, |_| None).unwrap();
    assert_eq!("$FOO", &expanded);
}

#[test]
fn expand_variable_alongside_braces() {
    let line = "$A{1,2}";
    let expected = "11 12";
    let expanded = expand_string(line, |_| None, |variable| {
        if variable == "A" { Some("1".to_owned()) } else { None }
    }).unwrap();
    assert_eq!(expected, &expanded);
}

#[test]
fn expand_variable_within_braces() {
    let line = "1{$A,2}";
    let expected = "11 12";
    let expanded = expand_string(line, |_| None, |variable| {
        if variable == "A" { Some("1".to_owned()) } else { None }
    }).unwrap();
    assert_eq!(expected, &expanded);
}
