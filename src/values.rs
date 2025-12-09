use std::{any::Any, collections::{HashMap, HashSet}, fmt::{Debug, Display, Write}, rc::Rc};

use crate::lexing::{Token, TokenType};
use crate::types::*;

#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    Any(Box<Value>),
    Int(i64),
    Float(f64),
    Char(char),
    Bool(bool),
    // Note that .clone is on the REFERENCE of objects
    // PartialEq compares object values, though
    String(Rc<String>),
    Function(Rc<Function>),
    NativeFunc(Rc<NativeFunction>),
    None,
}

// pub enum TypedValue {
//     Int(i64),
//     Float(f64),
//     Char(char),
//     Bool(bool),
//     // Note that .clone is on the REFERENCE of the object
//     // PartialEq compares object values, though
//     Object(Rc<Object>),
//     None,
// }

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::None => f.write_str("none"),
            Value::Any(value) => Display::fmt(&value, f),
            Value::Int(i) => f.write_str(&i.to_string()),
            Value::Float(flt) => f.write_str(&flt.to_string()),
            Value::Char(c) => f.write_char(*c),
            Value::Bool(b) => f.write_str(if *b { "true" } else { "false" }),
            Value::String(s) => f.write_str(s),
            Value::Function(function) => {
                let Function { name, ret_type, .. } = &**function;
                write!(f, "<func {name}(): {ret_type}>")
            }
            Value::NativeFunc(native) => {
                let NativeFunction { name, ret_type, .. } = &**native;
                write!(f, "<func {name}(): {ret_type}>")
            }
        }
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::String(Rc::new(value))
    }
}

impl From<NativeFunction> for Value {
    fn from(value: NativeFunction) -> Self {
        Self::NativeFunc(Rc::new(value))
    }
}

impl Value {
    pub fn get_type(&self) -> ValueType {
        match self {
            Value::None => ValueType::None,
            Value::Any(_) => ValueType::Any,
            Value::Int(_) => ValueType::Int,
            Value::Float(_) => ValueType::Float,
            Value::Char(_) => ValueType::Char,
            Value::Bool(_) => ValueType::Bool,
            Value::String(_) => ValueType::String,
            Value::Function(function) => {
                let Function { params, ret_type, .. } = &**function;
                ValueType::Function { ret_type: Box::new(ret_type.clone()), params: params.clone() }
            }
            Value::NativeFunc(function) => {
                let NativeFunction { params, ret_type, .. } = &**function;
                ValueType::Function { ret_type: Box::new(ret_type.clone()), params: params.clone().into() }
            }
        }
    }

    pub fn is_type(&self, val_type: &ValueType) -> bool {
        self.get_type() == *val_type
    }

}

#[derive(Debug, PartialEq)]
pub struct Function {
    pub name: String,
    pub params: Box<[ValueType]>,
    pub ret_type: ValueType,
    pub constants: Box<[Value]>,
    pub code: Box<[u8]>
}

#[derive(Debug, PartialEq)]
pub struct NativeFunction {
    pub name: &'static str,
    pub params: &'static [ValueType],
    pub ret_type: ValueType,
    pub func: fn(&[Value]) ->  Value,
}