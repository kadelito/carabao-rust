use std::{cell::RefCell, mem::ManuallyDrop, rc::Rc};

use crate::codegen::LineRLE;

// TODO unions

pub union RuntimeValue {
    any: ManuallyDrop<Box<RuntimeValue>>,
    // None
    int: i64,
    float: f64,
    char: char,
    bool: bool,
    string: ManuallyDrop<Rc<[u16]>>,
    range: ManuallyDrop<Rc<(RuntimeValue, RuntimeValue)>>,
    function: ManuallyDrop<Rc<CarabaoFunction>>,
    native_func: ManuallyDrop<Rc<NativeFunction>>,
    list: ManuallyDrop<Rc<RefCell<Vec<RuntimeValue>>>>,
}

pub struct CarabaoFunction {
    pub name: String,
    pub constants: Box<[RuntimeValue]>,
    pub code: Box<u8>,
    pub lines: Box<[LineRLE]>
}

pub struct NativeFunction {
    name: String,
    func: fn(&[RuntimeValue]) -> RuntimeValue,
}