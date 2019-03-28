use super::{super::types, Value};

// ***************************
//          Addition         *
// ***************************

#[test]
fn add_integer_integer() {
    let a = Value::Str("1".into());
    assert_eq!(&a + 2, Ok(Value::Str("3".into())));
    assert_eq!(&a + -2, Ok(Value::Str("-1".into())));
    assert_eq!(&a + 0, Ok(Value::Str("1".into())));
}

#[test]
fn add_float_integer() {
    let a = Value::Str("1.2".into());
    assert_eq!(&a + 2, Ok(Value::Str("3.2".into())));
    assert_eq!(&a + -2, Ok(Value::Str("-0.8".into())));
    assert_eq!(&a + 0, Ok(Value::Str("1.2".into())));
}

#[test]
fn add_integer_float() {
    let a = Value::Str("1".into());
    assert_eq!(&a + 2.3, Ok(Value::Str("3.3".into())));
    // Floating point artifacts
    assert_eq!(&a + -2.3, Ok(Value::Str("-1.2999999999999998".into())));
    assert_eq!(&a + 0., Ok(Value::Str("1".into())));
}

#[test]
fn add_float_float() {
    let a = Value::Str("1.2".into());
    assert_eq!(&a + 2.8, Ok(Value::Str("4".into())));
    // Floating point artifacts
    assert_eq!(&a + -2.2, Ok(Value::Str("-1.0000000000000002".into())));
    assert_eq!(&a + 0, Ok(Value::Str("1.2".into())));
}

#[test]
fn add_array_integer() {
    let a = Value::Array(array![types::Str::from("1.2"), types::Str::from("1")]);
    assert_eq!(&a + 2, Ok(Value::Array(array![types::Str::from("3.2"), types::Str::from("3")])));
}

#[test]
fn add_array_float() {
    let a = Value::Array(array![types::Str::from("1.2"), types::Str::from("1")]);
    assert_eq!(&a + 2.8, Ok(Value::Array(array![types::Str::from("4"), types::Str::from("3.8")])));
}

#[test]
fn add_var_var_str() {
    let a = Value::Str("1.2".into());
    assert_eq!(&a + &Value::Str("2.8".into()), Ok(Value::Str("4".into())));
    assert_eq!(&a + &Value::Str("2".into()), Ok(Value::Str("3.2".into())));
}

#[test]
fn add_var_var_array() {
    let a = Value::Array(array![types::Str::from("1.2"), types::Str::from("1")]);
    assert_eq!(
        &a + &Value::Str("2.8".into()),
        Ok(Value::Array(array![types::Str::from("4"), types::Str::from("3.8")]))
    );
}

// ***************************
//        Substraction       *
// ***************************

#[test]
fn sub_integer_integer() {
    let a = Value::Str("1".into());
    assert_eq!(&a - 2, Ok(Value::Str("-1".into())));
    assert_eq!(&a - -2, Ok(Value::Str("3".into())));
    assert_eq!(&a - 0, Ok(Value::Str("1".into())));
}

#[test]
fn sub_float_integer() {
    let a = Value::Str("1.2".into());
    assert_eq!(&a - 2, Ok(Value::Str("-0.8".into())));
    assert_eq!(&a - -2, Ok(Value::Str("3.2".into())));
    assert_eq!(&a - 0, Ok(Value::Str("1.2".into())));
}

#[test]
fn sub_integer_float() {
    let a = Value::Str("1".into());
    // Floating point artifacts
    assert_eq!(&a - 2.3, Ok(Value::Str("-1.2999999999999998".into())));
    assert_eq!(&a - -2.3, Ok(Value::Str("3.3".into())));
    assert_eq!(&a - 0., Ok(Value::Str("1".into())));
}

#[test]
fn sub_float_float() {
    let a = Value::Str("1.2".into());
    // Floating point artifacts
    assert_eq!(&a - 2.8, Ok(Value::Str("-1.5999999999999999".into())));
    assert_eq!(&a - -2.2, Ok(Value::Str("3.4000000000000004".into())));
    assert_eq!(&a - 0, Ok(Value::Str("1.2".into())));
}

#[test]
fn sub_array_integer() {
    let a = Value::Array(array![types::Str::from("1.2"), types::Str::from("1")]);
    assert_eq!(&a - 2, Ok(Value::Array(array![types::Str::from("-0.8"), types::Str::from("-1")])));
}

#[test]
fn sub_array_float() {
    let a = Value::Array(array![types::Str::from("1.2"), types::Str::from("1")]);
    assert_eq!(&a - -2.8, Ok(Value::Array(array![types::Str::from("4"), types::Str::from("3.8")])));
}

#[test]
fn sub_var_var_str() {
    let a = Value::Str("1.2".into());
    assert_eq!(&a - &Value::Str("-2.8".into()), Ok(Value::Str("4".into())));
    assert_eq!(&a - &Value::Str("2".into()), Ok(Value::Str("-0.8".into())));
}

#[test]
fn sub_var_var_array() {
    let a = Value::Array(array![types::Str::from("1.2"), types::Str::from("1")]);
    assert_eq!(
        &a - &Value::Str("-2.8".into()),
        Ok(Value::Array(array![types::Str::from("4"), types::Str::from("3.8")]))
    );
}

// ***************************
//       Multiplication      *
// ***************************

#[test]
fn mul_integer_integer() {
    let a = Value::Str("1".into());
    assert_eq!(&a * 2, Ok(Value::Str("2".into())));
    assert_eq!(&a * -2, Ok(Value::Str("-2".into())));
    assert_eq!(&a * 0, Ok(Value::Str("0".into())));
}

#[test]
fn mul_float_integer() {
    let a = Value::Str("1.2".into());
    assert_eq!(&a * 2, Ok(Value::Str("2.4".into())));
    assert_eq!(&a * -2, Ok(Value::Str("-2.4".into())));
    assert_eq!(&a * 0, Ok(Value::Str("0".into())));
}

#[test]
fn mul_integer_float() {
    let a = Value::Str("1".into());
    assert_eq!(&a * 2.3, Ok(Value::Str("2.3".into())));
    assert_eq!(&a * -2.3, Ok(Value::Str("-2.3".into())));
    assert_eq!(&a * 0., Ok(Value::Str("0".into())));
}

#[test]
fn mul_float_float() {
    let a = Value::Str("1.2".into());
    assert_eq!(&a * 2.8, Ok(Value::Str("3.36".into())));
    assert_eq!(&a * -2.2, Ok(Value::Str("-2.64".into())));
    assert_eq!(&a * 0, Ok(Value::Str("0".into())));
}

#[test]
fn mul_array_integer() {
    let a = Value::Array(array![types::Str::from("1.2"), types::Str::from("1")]);
    assert_eq!(&a * 2, Ok(Value::Array(array![types::Str::from("2.4"), types::Str::from("2")])));
}

#[test]
fn mul_array_float() {
    let a = Value::Array(array![types::Str::from("1.2"), types::Str::from("1")]);
    assert_eq!(
        &a * -2.8,
        Ok(Value::Array(array![types::Str::from("-3.36"), types::Str::from("-2.8")]))
    );
}

#[test]
fn mul_var_var_str() {
    let a = Value::Str("1.2".into());
    assert_eq!(&a * &Value::Str("-2.8".into()), Ok(Value::Str("-3.36".into())));
    assert_eq!(&a * &Value::Str("2".into()), Ok(Value::Str("2.4".into())));
}

#[test]
fn mul_var_var_array() {
    let a = Value::Array(array![types::Str::from("1.2"), types::Str::from("1")]);
    assert_eq!(
        &a * &Value::Str("-2.8".into()),
        Ok(Value::Array(array![types::Str::from("-3.36"), types::Str::from("-2.8")]))
    );
}

// ***************************
//          Division         *
// ***************************

#[test]
fn div_integer_integer() {
    let a = Value::Str("1".into());
    println!("{}", 1. / 2.);
    assert_eq!(&a / 2, Ok(Value::Str("0.5".into())));
    assert_eq!(&a / -2, Ok(Value::Str("-0.5".into())));
    assert_eq!(&a / 1, Ok(Value::Str("1".into())));
}

#[test]
fn div_float_integer() {
    let a = Value::Str("1.2".into());
    assert_eq!(&a / 2, Ok(Value::Str("0.6".into())));
    assert_eq!(&a / -2, Ok(Value::Str("-0.6".into())));
    assert_eq!(&a / 1, Ok(Value::Str("1.2".into())));
}

#[test]
fn div_integer_float() {
    let a = Value::Str("1".into());
    assert_eq!(&a / 2.5, Ok(Value::Str("0.4".into())));
    assert_eq!(&a / -2.5, Ok(Value::Str("-0.4".into())));
    assert_eq!(&a / 1., Ok(Value::Str("1".into())));
}

#[test]
fn div_float_float() {
    let a = Value::Str("1.2".into());
    assert_eq!(&a / 2.4, Ok(Value::Str("0.5".into())));
    assert_eq!(&a / -2.4, Ok(Value::Str("-0.5".into())));
    assert_eq!(&a / 1, Ok(Value::Str("1.2".into())));
}

#[test]
fn div_array_integer() {
    let a = Value::Array(array![types::Str::from("1.2"), types::Str::from("1")]);
    assert_eq!(&a / 2, Ok(Value::Array(array![types::Str::from("0.6"), types::Str::from("0.5")])));
}

#[test]
fn div_array_float() {
    let a = Value::Array(array![types::Str::from("1.2"), types::Str::from("1")]);
    assert_eq!(
        &a / -2.5,
        Ok(Value::Array(array![types::Str::from("-0.48"), types::Str::from("-0.4")]))
    );
}

#[test]
fn div_var_var_str() {
    let a = Value::Str("1.2".into());
    assert_eq!(&a / &Value::Str("-2.4".into()), Ok(Value::Str("-0.5".into())));
    assert_eq!(&a / &Value::Str("2".into()), Ok(Value::Str("0.6".into())));
}

#[test]
fn div_var_var_array() {
    let a = Value::Array(array![types::Str::from("1.2"), types::Str::from("1")]);
    assert_eq!(
        &a / &Value::Str("-2.5".into()),
        Ok(Value::Array(array![types::Str::from("-0.48"), types::Str::from("-0.4")]))
    );
}
