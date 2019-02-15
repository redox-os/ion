use std::fmt::{self, Display, Formatter};

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
    HashMap(Box<Primitive>),
    BTreeMap(Box<Primitive>),
    Indexed(String, Box<Primitive>),
}

impl Primitive {
    pub(crate) fn parse(data: &str) -> Option<Primitive> {
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
            kind => {
                fn parse_inner_hash_map(inner: &str) -> Option<Primitive> {
                    match inner {
                        "" => Some(Primitive::HashMap(Box::new(Primitive::Any))),
                        _ => Primitive::parse(inner).map(|p| Primitive::HashMap(Box::new(p))),
                    }
                }
                fn parse_inner_btree_map(inner: &str) -> Option<Primitive> {
                    match inner {
                        "" => Some(Primitive::BTreeMap(Box::new(Primitive::Any))),
                        _ => Primitive::parse(inner).map(|p| Primitive::BTreeMap(Box::new(p))),
                    }
                }

                let res = if kind.starts_with("hmap[") {
                    let kind = &kind[5..];
                    kind.rfind(']').map(|found| &kind[..found]).and_then(parse_inner_hash_map)
                } else if kind.starts_with("bmap[") {
                    let kind = &kind[5..];
                    kind.rfind(']').map(|found| &kind[..found]).and_then(parse_inner_btree_map)
                } else {
                    None
                };

                if let Some(data) = res {
                    data
                } else {
                    return None;
                }
            }
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
            Primitive::HashMap(ref kind) => match **kind {
                Primitive::Any | Primitive::Str => write!(f, "hmap[]"),
                ref kind => write!(f, "hmap[{}]", kind),
            },
            Primitive::BTreeMap(ref kind) => match **kind {
                Primitive::Any | Primitive::Str => write!(f, "bmap[]"),
                ref kind => write!(f, "bmap[{}]", kind),
            },
            Primitive::Indexed(_, ref kind) => write!(f, "{}", kind),
        }
    }
}
