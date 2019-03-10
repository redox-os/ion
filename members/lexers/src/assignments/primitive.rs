use std::fmt::{self, Display, Formatter};

/// A primitive defines the type that a requested value should satisfy.
#[derive(Debug, PartialEq, Clone)]
pub enum Primitive {
    Str,
    StrArray,
    Boolean,
    BooleanArray,
    Integer,
    IntegerArray,
    Float,
    FloatArray,
    HashMap(Box<Primitive>),
    BTreeMap(Box<Primitive>),
    Indexed(String, Box<Primitive>),
}

impl Primitive {
    pub(crate) fn parse(data: &str) -> Option<Primitive> {
        match data {
            "str" => Some(Primitive::Str),
            "[str]" => Some(Primitive::StrArray),
            "bool" => Some(Primitive::Boolean),
            "[bool]" => Some(Primitive::BooleanArray),
            "int" => Some(Primitive::Integer),
            "[int]" => Some(Primitive::IntegerArray),
            "float" => Some(Primitive::Float),
            "[float]" => Some(Primitive::FloatArray),
            kind => {
                let mut parts = kind.splitn(2, '[');
                let collection = parts.next()?;
                let inner = parts.next()?;
                if let (inner, "]") = inner.split_at(inner.len() - 1) {
                    let inner = Box::new(Primitive::parse(inner)?);
                    match collection {
                        "hmap" => Some(Primitive::HashMap(inner)),
                        "bmap" => Some(Primitive::BTreeMap(inner)),
                        _ => None,
                    }
                } else {
                    None
                }
            }
        }
    }
}

impl Display for Primitive {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            Primitive::Str => write!(f, "str"),
            Primitive::StrArray => write!(f, "[str]"),
            Primitive::Boolean => write!(f, "bool"),
            Primitive::BooleanArray => write!(f, "[bool]"),
            Primitive::Float => write!(f, "float"),
            Primitive::FloatArray => write!(f, "[float]"),
            Primitive::Integer => write!(f, "int"),
            Primitive::IntegerArray => write!(f, "[int]"),
            Primitive::HashMap(ref kind) => match **kind {
                Primitive::Str => write!(f, "hmap[]"),
                ref kind => write!(f, "hmap[{}]", kind),
            },
            Primitive::BTreeMap(ref kind) => match **kind {
                Primitive::Str => write!(f, "bmap[]"),
                ref kind => write!(f, "bmap[{}]", kind),
            },
            Primitive::Indexed(_, ref kind) => write!(f, "{}", kind),
        }
    }
}
