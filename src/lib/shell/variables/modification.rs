use super::Value;

pub trait Modifications {
    fn append(&mut self, val: Value) -> bool;
    fn prepend(&mut self, val: Value) -> bool;
}

impl Modifications for Value {
    fn append(&mut self, val: Value) -> bool {
        match self {
            Value::Array(ref mut lhs) => match val {
                Value::Array(rhs) => {
                    lhs.extend(rhs.into_iter());
                    true
                }
                Value::Str(rhs) => {
                    lhs.push(rhs);
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

    fn prepend(&mut self, val: Value) -> bool {
        match self {
            Value::Array(ref mut lhs) => match val {
                Value::Array(rhs) => {
                    lhs.insert_many(0, rhs);
                    true
                }
                Value::Str(rhs) => {
                    lhs.insert(0, rhs);
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
