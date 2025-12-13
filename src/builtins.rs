use std::rc::Rc;

use crate::values::{NativeFunction, Value};
use crate::types::*;

// TODO a better compile-time system for accessing globals than this
pub const STR_FUNC_INDEX: u8 = 0;

pub const GLOBAL_FUNCS: [(&'static str, NativeFunction); 5] = {
    [
        ("__str", NativeFunction {
            name: "str",
            params: &[ValueType::Any],
            ret_type: ValueType::String,
            func: natives::to_string,
        }),
        ("print", NativeFunction {
            name: "print",
            params: &[ValueType::Any],
            ret_type: ValueType::None,
            func: natives::print,
        }),
        ("println", NativeFunction {
            name: "println",
            params: &[ValueType::Any],
            ret_type: ValueType::None,
            func: natives::println,
        }),
        ("clock", NativeFunction {
            name: "clock",
            params: &[],
            ret_type: ValueType::Int,
            func: natives::clock,
        }),
        ("__debug_val", NativeFunction {
            name: "debug_val",
            params: &[ValueType::Any],
            ret_type: ValueType::String,
            func: natives::debug_str,
        }),
    ]
};

pub mod natives {
    use super::*;
    use std::io::Write;

    pub fn print(args: &[Value]) -> Value {
        print!("{}", args[0]);
        Value::None
    }

    pub fn println(args: &[Value]) -> Value {
        println!("{}", args[0]);
        Value::None
    }

    pub fn clock(_args: &[Value]) -> Value {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|dur| dur.as_millis() as i64)
            .unwrap_or(-1);
        return Value::Int(t);
    }

    pub fn to_string(args: &[Value]) -> Value {
        let mut buf = Vec::new();
        write!(buf, "{}", args[0]).expect("writing to buffer should not fail??");
        let str = String::from_utf8(buf)
            .expect("i dont know how utf-8 works");
        Value::from(str)
    }

    pub fn debug_str(args: &[Value]) -> Value {
        let Value::Any(inner) = &args[0] else { panic!() };
        format!("{:?}", inner).into()
    }
}