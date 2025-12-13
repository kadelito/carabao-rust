use std::{any::Any, cell::RefCell, collections::{HashMap, HashSet}, fmt::{Debug, Display, Write}, rc::Rc, u32};

use crate::{codegen::LineRLE, lexing::{Token, TokenType}};
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
    List(Rc<RefCell<Vec<Value>>>),
    // Struct(Rc<RefCell<[Value]>>),
    None,
}

#[derive(PartialEq)]
pub struct Function {
    pub name: String,
    pub params: Box<[ValueType]>,
    pub ret_type: ValueType,
    pub constants: Box<[Value]>,
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
    pub func: fn(&[Value]) ->  Value,
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
            Value::List(list) => {
                write!(f, "[{}]", list.borrow().iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
                )
            },
        }
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::String(Rc::new(value))
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::String(Rc::new(value.to_owned()))
    }
}

impl From<NativeFunction> for Value {
    fn from(value: NativeFunction) -> Self {
        Self::NativeFunc(Rc::new(value))
    }
}

impl From<Vec<Value>> for Value {
    fn from(value: Vec<Value>) -> Self {
        Self::List(Rc::new(RefCell::new(value)))
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
            Value::List(list) => todo!(),
        }
    }

    pub fn is_type(&self, val_type: &ValueType) -> bool {
        self.get_type() == *val_type
    }

}
