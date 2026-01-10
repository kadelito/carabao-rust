use crate::{standard_library::registry::macros::val_into, values::TypedValue};

pub fn print(args: &[TypedValue]) -> TypedValue {
    let typed = val_into!(&args[0] => Any);
    print!("{typed}"); // we need a type to know how to format
    TypedValue::None
}

pub fn println(args: &[TypedValue]) -> TypedValue {
    let typed = val_into!(&args[0] => Any);
    println!("{typed}");
    TypedValue::None
}

pub fn clock(_args: &[TypedValue]) -> TypedValue {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|dur| dur.as_millis() as i64)
        .unwrap_or(-1); // no idea what to do if it fails
    return TypedValue::Int(t);
}