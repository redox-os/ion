use std::fmt::{self, Display, Formatter};

/// Keys are used in assignments to define which variable will be set, and whether the correct
/// types are being assigned.
#[derive(Debug, PartialEq, Clone)]
pub(crate) struct Key<'a> {
    pub kind: Primitive,
    pub name: &'a str,
}

/// Functions require that their keys to have a longer lifetime, and that is made possible
/// by eliminating the lifetime requirements via allocating a `String`.
#[derive(Debug, PartialEq, Clone)]
pub(crate) struct KeyBuf {
    pub kind: Primitive,
    pub name: String,
}

#[derive(Debug, PartialEq)]
pub(crate) enum TypeError<'a> {
    Invalid(&'a str),
    BadValue(Primitive),
}

impl<'a> Display for TypeError<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            TypeError::Invalid(parm) => write!(f, "invalid type supplied: {}", parm),
            TypeError::BadValue(ref expected) => write!(f, "expected {}", expected),
        }
    }
}

impl<'a> Key<'a> {
    fn new(name: &'a str, data: &'a str) -> Result<Key<'a>, TypeError<'a>> {
        match Primitive::parse(data) {
            Some(data) => Ok(Key { kind: data, name }),
            None => Err(TypeError::Invalid(data)),
        }
    }
}

impl<'a> From<Key<'a>> for KeyBuf {
    fn from(key: Key<'a>) -> KeyBuf {
        KeyBuf {
            kind: key.kind,
            name: key.name.to_owned(),
        }
    }
}

/// A primitive defines the type that a requested value should satisfy.
#[derive(Debug, PartialEq, Clone)]
pub enum Primitive {
    Any,
    AnyArray,
    Str,
    StrArray,
    Boolean,
    BooleanArray,
    Integer,
    IntegerArray,
    Float,
    FloatArray,
    Indexed(String, Box<Primitive>),
}

impl Primitive {
    fn parse(data: &str) -> Option<Primitive> {
        let data = match data {
            "[]" => Primitive::AnyArray,
            "str" => Primitive::Str,
            "str[]" => Primitive::StrArray,
            "bool" => Primitive::Boolean,
            "bool[]" => Primitive::BooleanArray,
            "int" => Primitive::Integer,
            "int[]" => Primitive::IntegerArray,
            "float" => Primitive::Float,
            "float[]" => Primitive::FloatArray,
            _ => return None,
        };
        Some(data)
    }
}

impl Display for Primitive {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            Primitive::Any | Primitive::Str => write!(f, "str"),
            Primitive::AnyArray => write!(f, "[]"),
            Primitive::Boolean => write!(f, "bool"),
            Primitive::BooleanArray => write!(f, "bool[]"),
            Primitive::Float => write!(f, "float"),
            Primitive::FloatArray => write!(f, "float[]"),
            Primitive::Integer => write!(f, "int"),
            Primitive::IntegerArray => write!(f, "int[]"),
            Primitive::StrArray => write!(f, "str[]"),
            Primitive::Indexed(_, ref kind) => write!(f, "{}", kind),
        }
    }
}

/// Quite simply, an iterator that returns keys.
#[derive(Debug, PartialEq)]
pub(crate) struct KeyIterator<'a> {
    data: &'a str,
    read: usize,
}

impl<'a> KeyIterator<'a> {
    // Executes when a semicolon was not found, but an array character was.
    fn parse_array(&mut self, name: &'a str) -> Result<Key<'a>, TypeError<'a>> {
        let index_ident_start = self.read;
        loop {
            let mut eol = self.read + 1 >= self.data.len();

            if self.data.as_bytes()[self.read] == b']' && (eol || self.data.as_bytes()[self.read + 1] == b' ') {
                let kind = match &self.data[index_ident_start..self.read] {
                    "" => Primitive::AnyArray,
                    s => Primitive::Indexed(s.to_owned(), Box::new(Primitive::Any)),
                };
                self.read += 1;

                break Ok(Key { name, kind });
            } else if self.data.as_bytes()[self.read] == b']' && self.data.as_bytes()[self.read + 1] == b':' {
                let index_ident_end = self.read;

                self.read += 2;

                while !eol && self.data.as_bytes()[self.read] != b' ' {
                    self.read += 1;
                    eol = self.read >= self.data.len();
                }

                let kind = match &self.data[index_ident_start..index_ident_end] {
                    "" => Primitive::AnyArray,
                    s => match Primitive::parse(&self.data[index_ident_end + 2..self.read]) {
                        Some(kind) => Primitive::Indexed(s.to_owned(), Box::new(kind)),
                        None => break Err(TypeError::Invalid(&self.data[index_ident_end + 1..self.read])),
                    }
                };

                break Ok(Key { name, kind });
            } else if !eol {
                self.read += 1;
            } else {
                break Err(TypeError::Invalid(&self.data[self.read..]));
            }
        }
    }

    // Parameters are values that follow the semicolon (':').
    fn parse_parameter(&mut self, name: &'a str) -> Result<Key<'a>, TypeError<'a>> {
        let mut start = self.read;
        for byte in self.data.bytes().skip(self.read) {
            self.read += 1;
            match byte {
                b' ' if start + 1 == self.read => start += 1,
                b' ' => return Key::new(name, &self.data[start..self.read].trim()),
                _ => (),
            }
        }

        if start == self.read {
            Err(TypeError::Invalid(""))
        } else {
            Key::new(name, &self.data[start..self.read].trim())
        }
    }

    pub(crate) fn new(data: &'a str) -> KeyIterator<'a> { KeyIterator { data, read: 0 } }
}

impl<'a> Iterator for KeyIterator<'a> {
    type Item = Result<Key<'a>, TypeError<'a>>;

    fn next(&mut self) -> Option<Result<Key<'a>, TypeError<'a>>> {
        let mut start = self.read;
        for byte in self.data.bytes().skip(self.read) {
            self.read += 1;
            match byte {
                b' ' if start + 1 == self.read => start += 1,
                b' ' => {
                    return Some(Ok(Key {
                        name: &self.data[start..self.read].trim(),
                        kind: Primitive::Any,
                    }))
                }
                b':' => {
                    return Some(self.parse_parameter(&self.data[start..self.read - 1].trim()));
                }
                b'[' => {
                    return Some(self.parse_array(&self.data[start..self.read - 1].trim()));
                }
                _ => (),
            }
        }
        if start == self.read {
            None
        } else {
            Some(Ok(Key {
                name: &self.data[start..self.read].trim(),
                kind: Primitive::Any,
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_parsing() {
        let mut parser = KeyIterator::new("a:int b[] c:bool d e:int[] f[0] g[$index] h[1]:int d:a");
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key {
                name: "a",
                kind: Primitive::Integer,
            },)
        );
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key {
                name: "b",
                kind: Primitive::AnyArray,
            },)
        );
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key {
                name: "c",
                kind: Primitive::Boolean,
            },)
        );
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key {
                name: "d",
                kind: Primitive::Any,
            },)
        );
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key {
                name: "e",
                kind: Primitive::IntegerArray,
            },)
        );
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key {
                name: "f",
                kind: Primitive::Indexed(String::from("0"), Box::new(Primitive::Any)),
            },)
        );
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key {
                name: "g",
                kind: Primitive::Indexed(String::from("$index"), Box::new(Primitive::Any)),
            },)
        );
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key {
                name: "h",
                kind: Primitive::Indexed(String::from("1"), Box::new(Primitive::Integer)),
            },)
        );
        assert_eq!(parser.next().unwrap(), Err(TypeError::Invalid("a")));
    }
}
