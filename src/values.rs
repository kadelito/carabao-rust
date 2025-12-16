use std::{any::Any, cell::RefCell, collections::{HashMap, HashSet}, fmt::{Debug, Display, Write, write}, mem::ManuallyDrop, rc::Rc, u32};

use crate::{codegen::LineRLE, lexing::{Token, TokenType}};
use crate::types::*;

#[derive(Debug, PartialEq, Clone)]
pub enum TypedValue {
    Any(Box<TypedValue>),
    Int(i64),
    Float(f64),
    Char(char),
    Bool(bool),
    // Note that .clone is on the REFERENCE of objects
    // PartialEq compares object values, though
    String(Rc<String>),
    Function(Rc<Function>),
    NativeFunc(Rc<NativeFunction>),
    List(Rc<RefCell<Vec<TypedValue>>>),
    // Class(Rc<RefCell<[Value]>>),
    None,
}

// TODO this
pub union RuntimeValue {
    any: ManuallyDrop<Box<TypedValue>>,
    // None
    int: i64,
    float: f64,
    char: char,
    bool: bool,
}

#[derive(PartialEq)]
pub struct Function {
    pub name: String,
    pub params: Box<[ValueType]>,
    pub ret_type: ValueType,
    pub constants: Box<[TypedValue]>,
    pub code: Box<[u8]>,
    pub lines: Box<[LineRLE]>
}

impl Function {
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

impl Debug for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            name,
            params,
            ret_type,
            ..
        } = self;
        // so func sqrt[float]: sqrt
        write!(f, "{:?}{:?} -> {:?}",
            name,
            params,
            ret_type,
        )
    }
}

#[derive(PartialEq)]
pub struct NativeFunction {
    pub name: &'static str,
    pub params: &'static [ValueType],
    pub ret_type: ValueType,
    pub func: fn(&[TypedValue]) ->  TypedValue,
}

impl Debug for NativeFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            name,
            params,
            ret_type,
            ..
        } = self;
        // so func sqrt[float]: sqrt
        write!(f, "{:?}{:?} -> {:?}",
            name,
            params,
            ret_type,
        )
    }
}

impl Display for TypedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypedValue::None => f.write_str("none"),
            TypedValue::Any(value) => Display::fmt(&value, f),
            TypedValue::Int(i) => f.write_str(&i.to_string()),
            TypedValue::Float(flt) => write!(f, "{:3.}", flt), // TODO better float formatting than this
            TypedValue::Char(c) => f.write_char(*c),
            TypedValue::Bool(b) => f.write_str(if *b { "true" } else { "false" }),
            TypedValue::String(s) => f.write_str(s),
            TypedValue::Function(function) => {
                let Function { name, ret_type, .. } = &**function;
                write!(f, "<func {name}(): {ret_type}>")
            }
            TypedValue::NativeFunc(native) => {
                let NativeFunction { name, ret_type, .. } = &**native;
                write!(f, "<func {name}(): {ret_type}>")
            }
            TypedValue::List(list) => {
                write!(f, "[{}]", list.borrow().iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
                )
            },
        }
    }
}

impl From<String> for TypedValue {
    fn from(value: String) -> Self {
        Self::String(Rc::new(value))
    }
}

impl From<&str> for TypedValue {
    fn from(value: &str) -> Self {
        Self::String(Rc::new(value.to_owned()))
    }
}

impl From<NativeFunction> for TypedValue {
    fn from(value: NativeFunction) -> Self {
        Self::NativeFunc(Rc::new(value))
    }
}

impl From<Vec<TypedValue>> for TypedValue {
    fn from(value: Vec<TypedValue>) -> Self {
        Self::List(Rc::new(RefCell::new(value)))
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
            TypedValue::Function(function) => {
                let Function { params, ret_type, .. } = &**function;
                ValueType::Function(FunctionType {ret_type: ret_type.clone(), params: params.clone()}.into())
            }
            TypedValue::NativeFunc(function) => {
                let NativeFunction { params, ret_type, .. } = &**function;
                ValueType::Function(FunctionType {ret_type: ret_type.clone(), params: (*params).into()}.into())
            }
            TypedValue::List(list) => ValueType::List(Box::new(
                if let Some(first) = list.borrow().first() {
                    first.get_type()
                } else {
                    ValueType::Any
                }
            )),
        }
    }

    pub fn is_type(&self, val_type: &ValueType) -> bool {
        self.get_type() == *val_type
    }

}
