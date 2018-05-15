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
            TypeError::BadValue(expected) => write!(f, "expected {}", expected),
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
#[derive(Debug, PartialEq, Copy, Clone)]
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
        let start = self.read;
        for byte in self.data.bytes().skip(self.read) {
            if byte == b' ' {
                match &self.data[start..self.read] {
                    "]" => {
                        return Ok(Key {
                            name,
                            kind: Primitive::AnyArray,
                        })
                    }
                    data @ _ => return Err(TypeError::Invalid(data)),
                }
            }
            self.read += 1;
        }

        match &self.data[start..] {
            "]" => {
                return Ok(Key {
                    name,
                    kind: Primitive::AnyArray,
                })
            }
            data @ _ => return Err(TypeError::Invalid(data)),
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
                    // NOTE: Borrowck issue?
                    let read = self.read;
                    return Some(self.parse_parameter(&self.data[start..read - 1].trim()));
                }
                b'[' => {
                    // NOTE: Borrowck issue?
                    let read = self.read;
                    return Some(self.parse_array(&self.data[start..read - 1].trim()));
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
        let mut parser = KeyIterator::new("a:int b[] c:bool d e:int[] d:a");
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key {
                name: "a",
                kind: Primitive::Integer,
            })
        );
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key {
                name: "b",
                kind: Primitive::AnyArray,
            })
        );
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key {
                name: "c",
                kind: Primitive::Boolean,
            })
        );
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key {
                name: "d",
                kind: Primitive::Any,
            })
        );
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key {
                name: "e",
                kind: Primitive::IntegerArray,
            })
        );
        assert_eq!(parser.next().unwrap(), Err(TypeError::Invalid("a")));
    }
}
