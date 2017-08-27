use std::fmt::{self, Display, Formatter};

#[derive(Debug, PartialEq, Clone)]
pub struct TypeArg<'a> {
    pub kind: Primitive,
    pub name: &'a str,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypeArgBuf {
    pub kind: Primitive,
    pub name: String,
}

#[derive(Debug, PartialEq)]
pub enum TypeError<'a> {
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

impl<'a> TypeArg<'a> {
    fn new(name: &'a str, data: &'a str) -> Result<TypeArg<'a>, TypeError<'a>> {
        match Primitive::parse(data) {
            Some(data) => Ok(TypeArg { kind: data, name }),
            None => Err(TypeError::Invalid(data)),
        }
    }
}

impl<'a> From<TypeArg<'a>> for TypeArgBuf {
    fn from(typearg: TypeArg<'a>) -> TypeArgBuf {
        TypeArgBuf {
            kind: typearg.kind,
            name: typearg.name.to_owned(),
        }
    }
}

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

#[derive(Debug, PartialEq)]
pub struct TypeParser<'a> {
    data: &'a str,
    read: usize,
}

impl<'a> TypeParser<'a> {
    pub fn new(data: &'a str) -> TypeParser<'a> { TypeParser { data, read: 0 } }

    fn parse_parameter(&mut self, name: &'a str) -> Result<TypeArg<'a>, TypeError<'a>> {
        let mut start = self.read;
        for byte in self.data.bytes().skip(self.read) {
            self.read += 1;
            match byte {
                b' ' if start + 1 == self.read => start += 1,
                b' ' => return TypeArg::new(name, &self.data[start..self.read].trim()),
                _ => (),
            }
        }

        if start == self.read {
            Err(TypeError::Invalid(""))
        } else {
            TypeArg::new(name, &self.data[start..self.read].trim())
        }
    }

    fn parse_array(&mut self, name: &'a str) -> Result<TypeArg<'a>, TypeError<'a>> {
        let start = self.read - 1;
        for byte in self.data.bytes().skip(self.read) {
            if byte == b' ' {
                match &self.data[start..self.read] {
                    "[]" => {
                        return Ok(TypeArg {
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
            "[]" => {
                return Ok(TypeArg {
                    name,
                    kind: Primitive::AnyArray,
                })
            }
            data @ _ => return Err(TypeError::Invalid(data)),
        }
    }
}

impl<'a> Iterator for TypeParser<'a> {
    type Item = Result<TypeArg<'a>, TypeError<'a>>;
    fn next(&mut self) -> Option<Result<TypeArg<'a>, TypeError<'a>>> {
        let mut start = self.read;
        for byte in self.data.bytes().skip(self.read) {
            self.read += 1;
            match byte {
                b' ' if start + 1 == self.read => start += 1,
                b' ' => {
                    return Some(Ok(TypeArg {
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
            Some(Ok(TypeArg {
                name: &self.data[start..self.read].trim(),
                kind: Primitive::Any,
            }))
        }
    }
}
