use std::{
    cell::{RefCell, UnsafeCell},
    mem::ManuallyDrop,
    rc::Rc,
};

use crate::{
    codegen::LineRLE,
    typed_values::{TypedFunction, TypedNativeFunction, TypedValue},
};

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
    list: ManuallyDrop<MyMutRc<Vec<RuntimeValue>>>,
}

impl From<TypedValue> for RuntimeValue {
    fn from(value: TypedValue) -> Self {
        match value {
            TypedValue::None => RuntimeValue { bool: false },
            TypedValue::Any(typed_value) => RuntimeValue {
                any: ManuallyDrop::new(Box::new((*typed_value).into())),
            },
            TypedValue::Int(int) => RuntimeValue { int },
            TypedValue::Float(float) => RuntimeValue { float },
            TypedValue::Char(char) => RuntimeValue { char },
            TypedValue::Bool(bool) => RuntimeValue { bool },
            TypedValue::String(str) => RuntimeValue {
                string: ManuallyDrop::new(str),
            },
            TypedValue::Range(range) => RuntimeValue {
                range: ManuallyDrop::new(Rc::new((range.0.clone().into(), range.1.clone().into()))),
            },
            TypedValue::Function(func) => todo!(),
            TypedValue::NativeFunc(func) => todo!(),
            TypedValue::List(list) => todo!(),
        }
    }
}

pub struct CarabaoFunction {
    pub name: Box<str>,
    pub constants: Box<[RuntimeValue]>,
    pub code: Box<[u8]>,
    pub lines: Box<[LineRLE]>,
}

impl From<TypedFunction> for CarabaoFunction {
    fn from(value: TypedFunction) -> Self {
        let TypedFunction {
            name,
            constants,
            code,
            lines,
            ..
        } = value;
        Self {
            name,
            constants: constants
                .into_iter()
                .map(|typed| typed.into())
                .collect::<Vec<RuntimeValue>>()
                .into_boxed_slice(),
            code,
            lines,
        }
    }
}

pub struct NativeFunction {
    name: &'static str,
    func: fn(&[RuntimeValue]) -> RuntimeValue,
}

impl From<TypedNativeFunction> for NativeFunction {
    fn from(value: TypedNativeFunction) -> Self {
        let TypedNativeFunction { name, func, .. } = value;
        NativeFunction { name, func: todo!() }
    }
}

pub struct MyMutRc<T> {
    inner: Rc<UnsafeCell<T>>,
}

impl<T> MyMutRc<T> {
    pub fn new(value: T) -> Self {
        MyMutRc { inner: Rc::new(UnsafeCell::new(value)) }
    }

    #[inline]
    pub fn get(&self) -> &T {
        unsafe { &*self.inner.get() }
    }

    #[inline]
    pub fn get_mut(&self) -> &mut T {
        unsafe { &mut *self.inner.get() }
    }
}

impl<T> From<T> for MyMutRc<T> {
    fn from(value: T) -> Self {
        MyMutRc { inner: Rc::new(UnsafeCell::new(value)) }
    }
}