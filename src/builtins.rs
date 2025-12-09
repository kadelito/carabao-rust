use std::rc::Rc;

use crate::values::{NativeFunction, Value};
use crate::types::*;

//
pub const GLOBAL_FUNCS: [(&'static str, NativeFunction); 2] = {
    [
        ("print", NativeFunction {
            name: "print",
            params: &[ValueType::Any],
            ret_type: ValueType::None,
            func: print_native,
        }),
        ("println", NativeFunction {
            name: "println",
            params: &[ValueType::Any],
            ret_type: ValueType::None,
            func: println_native,
        }),
    ]
};

pub fn print_native(args: &[Value]) -> Value {
    print!("{}", args[0]);
    Value::None
}

pub fn println_native(args: &[Value]) -> Value {
    print!("{}\n", args[0]);
    Value::None
}