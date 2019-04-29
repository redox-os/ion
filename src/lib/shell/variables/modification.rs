use super::Value;

pub trait Modifications {
    fn append(&mut self, val: Self) -> bool;
    fn prepend(&mut self, val: Self) -> bool;
}

impl<'a> Modifications for Value<'a> {
    fn append(&mut self, val: Self) -> bool {
        match self {
            Value::Array(ref mut lhs) => match val {
                Value::Array(rhs) => {
                    lhs.extend(rhs);
                    true
                }
                Value::Str(_) => {
                    lhs.push(val);
                    true
                }
                _ => false,
            },
            Value::Str(ref mut lhs) => match val {
                Value::Str(rhs) => {
                    lhs.push_str(rhs.as_str());
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn prepend(&mut self, val: Self) -> bool {
        match self {
            Value::Array(ref mut lhs) => match val {
                Value::Array(rhs) => {
                    lhs.splice(..0, rhs);
                    true
                }
                Value::Str(_) => {
                    lhs.insert(0, val);
                    true
                }
                _ => false,
            },
            Value::Str(ref mut lhs) => match val {
                Value::Str(rhs) => {
                    *lhs = format!("{}{}", rhs, lhs).into();
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }
}
