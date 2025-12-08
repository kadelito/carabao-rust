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
    // Note that .clone is on the REFERENCE of the object
    // PartialEq compares object values, though
    Object(Rc<Object>),
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
            Value::Any(value) => Display::fmt(&value, f),
            Value::Int(i) => f.write_str(&i.to_string()),
            Value::Float(flt) => f.write_str(&flt.to_string()),
            Value::Char(c) => f.write_char(*c),
            Value::Bool(b) => f.write_str(if *b { "true" } else { "false" }),
            Value::Object(object) => std::fmt::Display::fmt(&object, f),
            Value::None => f.write_str("none")
        }
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::from(Object::String(value))
    }
}

impl From<NativeFunction> for Value {
    fn from(value: NativeFunction) -> Self {
        Self::from(Object::NativeFunc(value))
    }
}

impl From<Object> for Value {
    fn from(value: Object) -> Self {
        Self::Object(Rc::new(value))
    }
}

impl Value {
    pub fn get_type(&self) -> ValueType {
        match self {
            Value::Any(_) => ValueType::Any,
            Value::Int(_) => ValueType::Int,
            Value::Float(_) => ValueType::Float,
            Value::Char(_) => ValueType::Char,
            Value::Bool(_) => ValueType::Bool,
            Value::Object(obj) => ValueType::Object(obj.get_obj_type()),
            Value::None => ValueType::None,
        }
    }

    pub fn is_type(&self, val_type: &ValueType) -> bool {
        self.get_type() == *val_type
    }

    pub fn is_obj_type(&self, obj_type: &ObjectType) -> bool {
        self.get_type() == ValueType::Object(obj_type.clone())
    }

    pub fn cast(&self, new_type: &ValueType) -> Option<Value> {
        if self.is_type(new_type) {
            // already that type
            Some(self.clone())
        } else if let Value::Any(inner) = self {
            inner.cast(new_type)
        } else if *new_type == ValueType::None {
            Some(Value::None)
        } else if *new_type == ValueType::Any {
            // Don't wrap if already Any
            Some(Value::Any(if let Value::Any(inner) = self {
                inner.clone()
            } else {
                Box::new(self.clone())
            }))
        } else if *new_type == ValueType::Object(ObjectType::String) {
            Some(Value::from(self.to_string()))
        } else {
            match (self, new_type) {
                (Value::Int(i), ValueType::Float) => Some(Value::Float(*i as f64)),
                (Value::Int(i), ValueType::Char) => {
                    if let Some(c) = char::from_u32(*i as u32) {
                        Some(Value::Char(c))
                    } else {
                        None
                    }
                }
                (Value::Int(i), ValueType::Bool) => Some(Value::Bool(*i != 0)),
                (Value::Float(f), ValueType::Int) => Some(Value::Int(*f as i64)),
                (Value::Float(f), ValueType::Bool) => Some(Value::Bool(f64::is_finite(*f) && *f != 0.0)),
                (Value::Char(c), ValueType::Int) => Some(Value::Int(*c as i64)),
                (Value::Bool(b), ValueType::Int) => Some(Value::Int(if *b {1} else {0})),
                (Value::Bool(b), ValueType::Float) => Some(Value::Float(if *b {1.0} else {0.0})),
                (Value::None, ValueType::Int) => Some(Value::Int(0)),
                (Value::None, ValueType::Float) => Some(Value::Float(0.0)),
                (Value::None, ValueType::Char) => Some(Value::Char('\0')),
                (Value::None, ValueType::Bool) => Some(Value::Bool(false)),
                _ => None,
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Object {
    String(String),
    Function(Function),
    NativeFunc(NativeFunction),
}

impl Display for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Object::String(s) => f.write_str(s),
            Object::Function(function) => {
                let Function { name, ret_type, .. } = function;
                write!(f, "<func {name}(): {ret_type}>")
            },
            Object::NativeFunc(native) => {
                let NativeFunction { name, ret_type, .. } = native;
                write!(f, "<func {name}(): {ret_type}>")
            }
        }
    }
}

impl Clone for Object {
    fn clone(&self) -> Self {
        match self {
            Object::String(s) => Self::String(s.clone()),
            Object::Function(function) => todo!(),
            Object::NativeFunc(native_function) => todo!(),
        }
    }
}

impl Object {
    pub fn add(obj1: &Self, obj2: &Self) -> Option<Value> {
        match (obj1, obj2) {
            (Object::String(s1), Object::String(s2)) => Some(Value::from(s1.to_owned() + s2)),
            _ => None,
        }
    }

    pub fn get_obj_type(&self) -> ObjectType {
        match self {
            Object::String(_) => ObjectType::String,
            Object::Function(Function { params, ret_type, ..  }) => 
                ObjectType::Function { ret_type: Box::new(ret_type.clone()), params: params.clone(), },
            Object::NativeFunc(NativeFunction { params, ret_type, .. }) => 
                ObjectType::Function { ret_type: Box::new(ret_type.clone()), params: params.clone().into() },
        }
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