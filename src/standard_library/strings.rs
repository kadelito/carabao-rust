use crate::{standard_library::registry::macros::val_into, values::TypedValue};
use std::io::Write;

pub fn to_string(args: &[TypedValue]) -> TypedValue {
    let mut buf = Vec::new();
    write!(buf, "{}", args[0]).expect("writing to buffer should not fail??");
    let str = String::from_utf8(buf)
        .expect("i dont know how utf-8 works");
    TypedValue::from(str)
}

pub fn debug_str(args: &[TypedValue]) -> TypedValue {
    let TypedValue::Any(inner) = &args[0] else { panic!() };
    format!("{:?}", inner).into()
}

pub fn str_len(args: &[TypedValue]) -> TypedValue {
    let TypedValue::String(s) = &args[0] else { panic!() };
    TypedValue::Int(s.len() as i64)
}

pub fn str_concat(args: &[TypedValue]) -> TypedValue {
    let s1 = val_into!(&args[0] => String).clone();
    let s2 = val_into!(&args[1] => String).clone();
    TypedValue::String([s1, s2].concat().into_boxed_slice().into())
}