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
    Array(Box<Self>),
    /// A hash map
    HashMap(Box<Self>),
    /// A btreemap
    BTreeMap(Box<Self>),
    /// An index variable (ex: `$array[0]`)
    Indexed(String, Box<Self>),
}

impl Primitive {
    pub(crate) fn parse(data: &str) -> Option<Self> {
        match data {
            "str" => Some(Self::Str),
            "bool" => Some(Self::Boolean),
            "int" => Some(Self::Integer),
            "float" => Some(Self::Float),
            _ => {
                let open_bracket = data.find('[')?;
                let close_bracket = data.rfind(']')?;
                let kind = &data[..open_bracket];
                let inner = &data[open_bracket + 1..close_bracket];

                if kind == "hmap" {
                    Some(Self::HashMap(Box::new(Self::parse(inner)?)))
                } else if kind == "bmap" {
                    Some(Self::BTreeMap(Box::new(Self::parse(inner)?)))
                } else {
                    // It's an array
                    Some(Self::Array(Box::new(Self::parse(inner)?)))
                }
            }
        }
    }
}

impl Display for Primitive {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            Self::Str => write!(f, "str"),
            Self::Boolean => write!(f, "bool"),
            Self::Float => write!(f, "float"),
            Self::Integer => write!(f, "int"),
            Self::Array(ref kind) => write!(f, "[{}]", kind),
            Self::HashMap(ref kind) => match **kind {
                Self::Str => write!(f, "hmap[]"),
                ref kind => write!(f, "hmap[{}]", kind),
            },
            Self::BTreeMap(ref kind) => match **kind {
                Self::Str => write!(f, "bmap[]"),
                ref kind => write!(f, "bmap[{}]", kind),
            },
            Self::Indexed(_, ref kind) => write!(f, "{}", kind),
        }
    }
}
