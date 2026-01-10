use std::{
    cell::UnsafeCell,
    fmt::{Debug, Display, Write},
    rc::Rc,
    u32,
};

use crate::types::*;
use crate::{
    codegen::LineRLE,
    errors::macros::internal_error,
};

#[derive(Debug, PartialEq, Clone)]
pub enum TypedValue {
    Any(Box<TypedValue>),
    Int(i64),
    Float(f64),
    Char(char),
    Bool(bool),
    // Double pointer avoids Rc being fat
    // sorry cache :3
    String(Rc<Box<[u16]>>),
    Range(Rc<(TypedValue, TypedValue)>),
    Function(Rc<TypedFunction>),
    NativeFunc(Rc<TypedNativeFunction>),
    List(MutRc<Vec<TypedValue>>),
    Object(MutRc<[TypedValue]>),
    None,
}

#[derive(Debug)]
pub struct MutRc<T: ?Sized> {
    inner: Rc<UnsafeCell<T>>,
}

impl<T: ?Sized> MutRc<T> {
    pub fn get(&self) -> &T {
        unsafe { &*self.inner.get() }
    }

    pub fn get_mut(&self) -> &mut T {
        unsafe { &mut *self.inner.get() }
    }
}

impl<T: ?Sized> From<Box<T>> for MutRc<T> {
    fn from(value: Box<T>) -> Self {
        Self {
            inner: {
                let ptr = unsafe { &mut *Box::into_raw(value) };
                let cell = UnsafeCell::from_mut(ptr);
                let boxed = unsafe { Box::from_raw(cell) };
                boxed.into()
            },
        }
    }
}

// Sized version with much fewer
impl<T: Sized> From<T> for MutRc<T> {
    fn from(value: T) -> Self {
        Self {
            inner: Rc::new(UnsafeCell::new(value)),
        }
    }
}

impl<T: ?Sized> PartialEq for MutRc<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl<T: ?Sized> Clone for MutRc<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

#[derive(PartialEq)]
pub struct TypedFunction {
    pub name: Box<str>,
    pub params: Box<[ValueType]>,
    pub ret_type: ValueType,
    pub constants: Box<[TypedValue]>,
    pub code: Box<[u8]>,
    pub lines: Box<[LineRLE]>,
}

impl TypedFunction {
    pub fn get_line(&self, index: usize) -> u32 {
        if index == 0 {
            return self.lines[0].line;
        }

        let mut bytes_passed: usize = 0;
        for info in &self.lines {
            bytes_passed += info.count as usize;
            if bytes_passed > index {
                return info.line;
            }
        }
        if index >= bytes_passed {
            u32::MAX
        } else {
            return self.lines.last().unwrap().line;
        }
    }
}
impl Debug for TypedFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            name,
            params,
            ret_type,
            ..
        } = self;
        // so func sqrt[float]: sqrt
        write!(f, "{:?}{:?} -> {:?}", name, params, ret_type,)
    }
}

#[derive(PartialEq)]
pub struct TypedNativeFunction {
    pub name: &'static str,
    pub params: &'static [ValueType],
    pub ret_type: ValueType,
    pub func: fn(&[TypedValue]) -> TypedValue,
    // TODO replace native with result
    // pub func: fn(&[TypedValue]) -> Result<TypedValue, RuntimeError>,
}

impl Debug for TypedNativeFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            name,
            params,
            ret_type,
            ..
        } = self;
        // so func sqrt[float]: sqrt
        write!(f, "{:?}{:?} -> {:?}", name, params, ret_type,)
    }
}

impl Display for TypedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypedValue::None => f.write_str("none"),
            TypedValue::Any(value) => Display::fmt(&value, f),
            TypedValue::Int(i) => f.write_str(&i.to_string()),
            TypedValue::Float(flt) => write!(f, "{:.}", flt),
            TypedValue::Char(c) => f.write_char(*c),
            TypedValue::Bool(b) => f.write_str(if *b { "true" } else { "false" }),
            TypedValue::String(s) => f.write_str(&String::from_utf16_lossy(&s)),
            TypedValue::Range(range) => {
                let (start, end) = range.as_ref();
                write!(f, "[{start}...{end}]")
            }
            TypedValue::Function(function) => {
                let TypedFunction { name, ret_type, .. } = function.as_ref();
                write!(f, "<func {name}(): {ret_type}>")
            }
            TypedValue::NativeFunc(native) => {
                let TypedNativeFunction { name, ret_type, .. } = native.as_ref();
                write!(f, "<func {name}(): {ret_type}>")
            }
            TypedValue::List(list) => write!(
                f,
                "[{}]",
                list.get()
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            TypedValue::Object(obj) => write!(
                f,
                "{{{}}}",
                obj.get()
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
        }
    }
}

impl From<String> for TypedValue {
    fn from(value: String) -> Self {
        let utf16_chars = value
            .encode_utf16()
            .collect::<Vec<u16>>()
            .into_boxed_slice()
            .into();
        Self::String(utf16_chars)
    }
}

impl From<&str> for TypedValue {
    fn from(value: &str) -> Self {
        value.to_owned().into()
    }
}

impl From<TypedNativeFunction> for TypedValue {
    fn from(value: TypedNativeFunction) -> Self {
        Self::NativeFunc(Rc::new(value))
    }
}

impl From<Vec<TypedValue>> for TypedValue {
    fn from(value: Vec<TypedValue>) -> Self {
        Self::List(value.into())
    }
}

impl TypedValue {
    pub fn get_type(&self) -> ValueType {
        match self {
            TypedValue::None => ValueType::None,
            TypedValue::Any(_) => ValueType::Any,
            TypedValue::Int(_) => ValueType::Int,
            TypedValue::Float(_) => ValueType::Float,
            TypedValue::Char(_) => ValueType::Char,
            TypedValue::Bool(_) => ValueType::Bool,
            TypedValue::String(_) => ValueType::String,
            TypedValue::Range(range) => ValueType::Range(range.0.get_type().into()),
            TypedValue::Function(function) => {
                let TypedFunction {
                    params, ret_type, ..
                } = function.as_ref();
                ValueType::Function(
                    FunctionType {
                        ret_type: ret_type.clone(),
                        params: params.clone(),
                        native: false,
                    }
                    .into(),
                )
            }
            TypedValue::NativeFunc(function) => {
                let TypedNativeFunction {
                    params, ret_type, ..
                } = function.as_ref();
                ValueType::Function(
                    FunctionType {
                        ret_type: ret_type.clone(),
                        params: (*params).into(),
                        native: true,
                    }
                    .into(),
                )
            }
            TypedValue::List(list) => ValueType::List(
                if let Some(first) = list.get().first() {
                    first.get_type()
                } else {
                    ValueType::Any
                }
                .into(),
            ),
            TypedValue::Object(_) => internal_error!("Runtime object does not know its fields"),
        }
    }
}
