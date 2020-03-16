use std::fmt::{self, Display, Formatter};

/// A primitive defines the type that a requested value should satisfy.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Primitive {
    /// A plain string (ex: `"a string"`)
    Str,
    /// A true-false value
    Boolean,
    /// An integer numeric type
    Integer,
    /// A floating-point value
    Float,
    /// Arrays
    Array(Box<Primitive>),
    /// A hash map
    HashMap(Box<Primitive>),
    /// A btreemap
    BTreeMap(Box<Primitive>),
    /// An index variable (ex: `$array[0]`)
    Indexed(String, Box<Primitive>),
}

impl Primitive {
    pub(crate) fn parse(data: &str) -> Option<Self> {
        match data {
            "str" => Some(Primitive::Str),
            "bool" => Some(Primitive::Boolean),
            "int" => Some(Primitive::Integer),
            "float" => Some(Primitive::Float),
            _ => {
                let open_bracket = data.find('[')?;
                let close_bracket = data.rfind(']')?;
                let kind = &data[..open_bracket];
                let inner = &data[open_bracket + 1..close_bracket];

                if kind == "hmap" {
                    Some(Primitive::HashMap(Box::new(Self::parse(inner)?)))
                } else if kind == "bmap" {
                    Some(Primitive::BTreeMap(Box::new(Self::parse(inner)?)))
                } else {
                    // It's an array
                    Some(Primitive::Array(Box::new(Self::parse(inner)?)))
                }
            }
        }
    }
}

impl Display for Primitive {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            Primitive::Str => write!(f, "str"),
            Primitive::Boolean => write!(f, "bool"),
            Primitive::Float => write!(f, "float"),
            Primitive::Integer => write!(f, "int"),
            Primitive::Array(ref kind) => write!(f, "[{}]", kind),
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
