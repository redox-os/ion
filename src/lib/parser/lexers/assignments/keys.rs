use super::Primitive;
use err_derive::Error;

/// Keys are used in assignments to define which variable will be set, and whether the correct
/// types are being assigned.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Key<'a> {
    /// What should be the type of the literal
    pub kind: Primitive,
    /// What name should be given to the variable
    pub name: &'a str,
}

/// Functions require that their keys to have a longer lifetime, and that is made possible
/// by eliminating the lifetime requirements via allocating a `String`.
#[derive(Debug, PartialEq, Clone)]
pub struct KeyBuf {
    /// What type should the literal be
    pub kind: Primitive,
    /// What name should be given to the variable
    pub name: String,
}

/// Failed to parse the literal as a variable corresponding to key
#[derive(Debug, PartialEq, Error, Eq, Hash, Clone)]
pub enum TypeError {
    /// The value supplied is invalid
    #[error(display = "invalid type supplied: {}", _0)]
    Invalid(String),
    /// The primitive type does not correspond to that of the function argument
    #[error(display = "expected {}", _0)]
    BadValue(Primitive),
}

impl<'a> Key<'a> {
    fn parse(name: &'a str, data: &'a str) -> Result<Key<'a>, TypeError> {
        match Primitive::parse(data) {
            Some(data) => Ok(Key { kind: data, name }),
            None => Err(TypeError::Invalid(data.into())),
        }
    }
}

impl<'a> From<Key<'a>> for KeyBuf {
    fn from(key: Key<'a>) -> Self { Self { kind: key.kind, name: key.name.to_owned() } }
}

/// Quite simply, an iterator that returns keys.
#[derive(Debug, PartialEq)]
pub struct KeyIterator<'a> {
    data: &'a str,
    read: usize,
}

impl<'a> KeyIterator<'a> {
    // Executes when a semicolon was not found, but an array character was.
    fn parse_array(&mut self, name: &'a str) -> Result<Key<'a>, TypeError> {
        let index_ident_start = self.read;
        loop {
            let mut eol = self.read + 1 >= self.data.len();

            if self.data.as_bytes()[self.read] == b']'
                && (eol || self.data.as_bytes()[self.read + 1] == b' ')
            {
                let kind = match &self.data[index_ident_start..self.read] {
                    "" => Primitive::Array(Box::new(Primitive::Str)),
                    s => Primitive::Indexed(s.to_owned(), Box::new(Primitive::Str)),
                };
                self.read += 1;

                break Ok(Key { name, kind });
            } else if self.data.as_bytes()[self.read] == b']'
                && self.data.as_bytes()[self.read + 1] == b':'
            {
                let index_ident_end = self.read;

                self.read += 2;

                while !eol && self.data.as_bytes()[self.read] != b' ' {
                    self.read += 1;
                    eol = self.read >= self.data.len();
                }

                let kind = match &self.data[index_ident_start..index_ident_end] {
                    "" => Primitive::Array(Box::new(Primitive::Str)),
                    s => match Primitive::parse(&self.data[index_ident_end + 2..self.read]) {
                        Some(kind) => Primitive::Indexed(s.to_owned(), Box::new(kind)),
                        None => {
                            break Err(TypeError::Invalid(
                                self.data[index_ident_end + 1..self.read].into(),
                            ));
                        }
                    },
                };

                break Ok(Key { name, kind });
            } else if eol {
                break Err(TypeError::Invalid(self.data[self.read..].into()));
            } else {
                self.read += 1;
            }
        }
    }

    // Parameters are values that follow the semicolon (':').
    fn parse_parameter(&mut self, name: &'a str) -> Result<Key<'a>, TypeError> {
        let mut start = self.read;
        for byte in self.data.bytes().skip(self.read) {
            self.read += 1;
            match byte {
                b' ' if start + 1 == self.read => start += 1,
                b' ' => return Key::parse(name, self.data[start..self.read].trim()),
                _ => (),
            }
        }

        if start == self.read {
            Err(TypeError::Invalid(String::new()))
        } else {
            Key::parse(name, self.data[start..self.read].trim())
        }
    }

    /// Create a new iterator based on given data
    pub const fn new(data: &'a str) -> KeyIterator<'a> { KeyIterator { data, read: 0 } }
}

impl<'a> Iterator for KeyIterator<'a> {
    type Item = Result<Key<'a>, TypeError>;

    fn next(&mut self) -> Option<Result<Key<'a>, TypeError>> {
        let mut start = self.read;
        for byte in self.data.bytes().skip(self.read) {
            self.read += 1;
            match byte {
                b' ' if start + 1 == self.read => start += 1,
                b' ' => {
                    return Some(Ok(Key {
                        name: self.data[start..self.read].trim(),
                        kind: Primitive::Str,
                    }));
                }
                b':' => {
                    let end = self.read - 1;
                    return Some(self.parse_parameter(self.data[start..end].trim()));
                }
                b'[' => {
                    let end = self.read - 1;
                    return Some(self.parse_array(self.data[start..end].trim()));
                }
                _ => (),
            }
        }
        if start == self.read {
            None
        } else {
            Some(Ok(Key { name: self.data[start..self.read].trim(), kind: Primitive::Str }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_parsing() {
        let mut parser = KeyIterator::new(
            "a:int b[] c:bool d e:[int] f[0] g[$index] h[1]:int i:hmap[str] j:hmap[float] \
             k:hmap[[int]] l:hmap[hmap[[bool]]] m:bmap[str] n:bmap[int] o:bmap[[float]] \
             p:bmap[hmap[bool]] d:a",
        );
        assert_eq!(parser.next().unwrap(), Ok(Key { name: "a", kind: Primitive::Integer },));
        assert_eq!(parser.next().unwrap(), Ok(Key { name: "b", kind: Primitive::Array(Box::new(Primitive::Str)) },));
        assert_eq!(parser.next().unwrap(), Ok(Key { name: "c", kind: Primitive::Boolean },));
        assert_eq!(parser.next().unwrap(), Ok(Key { name: "d", kind: Primitive::Str },));
        assert_eq!(parser.next().unwrap(), Ok(Key { name: "e", kind: Primitive::Array(Box::new(Primitive::Integer)) },));
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key { name: "f", kind: Primitive::Indexed("0".into(), Box::new(Primitive::Str)) },)
        );
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key {
                name: "g",
                kind: Primitive::Indexed("$index".into(), Box::new(Primitive::Str)),
            },)
        );
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key {
                name: "h",
                kind: Primitive::Indexed("1".into(), Box::new(Primitive::Integer)),
            },)
        );
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key { name: "i", kind: Primitive::HashMap(Box::new(Primitive::Str)) },)
        );
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key { name: "j", kind: Primitive::HashMap(Box::new(Primitive::Float)) },)
        );
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key { name: "k", kind: Primitive::HashMap(Box::new(Primitive::Array(Box::new(Primitive::Integer)))) },)
        );
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key {
                name: "l",
                kind: Primitive::HashMap(Box::new(Primitive::HashMap(Box::new(
                    Primitive::Array(Box::new(Primitive::Boolean))
                )))),
            },)
        );
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key { name: "m", kind: Primitive::BTreeMap(Box::new(Primitive::Str)) },)
        );
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key { name: "n", kind: Primitive::BTreeMap(Box::new(Primitive::Integer)) },)
        );
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key { name: "o", kind: Primitive::BTreeMap(Box::new(Primitive::Array(Box::new(Primitive::Float)))) },)
        );
        assert_eq!(
            parser.next().unwrap(),
            Ok(Key {
                name: "p",
                kind: Primitive::BTreeMap(Box::new(Primitive::HashMap(Box::new(
                    Primitive::Boolean
                )))),
            },)
        );
        assert_eq!(parser.next().unwrap(), Err(TypeError::Invalid("a".into())));
    }
}
