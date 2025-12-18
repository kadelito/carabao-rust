use crate::values::TypedValue;

pub fn print(args: &[TypedValue]) -> TypedValue {
    print!("{}", args[0]);
    TypedValue::None
}

pub fn println(args: &[TypedValue]) -> TypedValue {
    println!("{}", args[0]);
    TypedValue::None
}

pub fn clock(_args: &[TypedValue]) -> TypedValue {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|dur| dur.as_millis() as i64)
        .unwrap_or(-1);
    return TypedValue::Int(t);
}