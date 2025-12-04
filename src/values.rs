use std::{any::Any, collections::{HashMap, HashSet}, fmt::Display, rc::Rc};

use crate::lexing::{Token, TokenType};

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

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::Object(Rc::new(Object::String(value)))
    }
}

impl ToString for Value {
    fn to_string(&self) -> String {
        match self {
            Value::Any(val) => format!("<Any {:?}>", val),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Char(c) => c.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Object(o) => o.to_string(),
            Value::None => "none".to_owned(),
        }
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
            Value::Object(obj) => ValueType::Object(match &**obj {
                                                            // rust i swear to god
                Object::String(_) => ObjectType::String,
                Object::Function(Function { params, ret_type, ..  }) => 
                    ObjectType::Function { ret_type: Box::new(ret_type.clone()), params: params.clone(), },
            }),
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

#[derive(Debug, PartialEq, Clone)]
pub enum ValueType {
    Any,
    Int,
    Float,
    Char,
    Bool,
    Object(ObjectType),
    None,
}

#[derive(Debug, PartialEq, Clone)]
pub enum ObjectType {
    String,
    /// this is just generics all over again
    Function { ret_type: Box<ValueType>, params: Box<[ValueType]> },
}

impl Display for ValueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Self::Object(obj_type) = self {
            obj_type.fmt(f)
        } else {
            f.write_str(match self {
                ValueType::Any => "any",
                ValueType::Int => "int",
                ValueType::Float => "float",
                ValueType::Char => "char",
                ValueType::Bool => "bool",
                ValueType::Object(_) => "",
                ValueType::None => "none",
            })
        }
    }
}

impl Display for ObjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ObjectType::String => "string",
            ObjectType::Function { .. } => "func",
        })
    }
}

impl ValueType {
    pub fn from(value: TokenType) -> Option<Self> {
        match value {
            TokenType::Any => Some(Self::Any),
            TokenType::Int => Some(Self::Int),
            TokenType::Float => Some(Self::Float),
            TokenType::Char => Some(Self::Char),
            TokenType::Bool => Some(Self::Bool),
            TokenType::String => Some(Self::Object(ObjectType::String)),
            TokenType::None => Some(Self::None),
            _ => None,
        }
    }

    pub fn coerce(type1: &ValueType, type2: &ValueType) -> Option<(ValueType, ValueType)> {
        if type1 == type2 {
            // Already the same type
            Some((type1.clone(), (type2.clone())))
        } else if [type1, type2].contains(&&ValueType::Object(ObjectType::String)) {
            // val1 is a string
            Some((ValueType::Object(ObjectType::String), ValueType::Object(ObjectType::String)))
        } else {
            match (type1, type2) {
                (ValueType::Int, ValueType::Float) => Some((ValueType::Float, ValueType::Float)),
                (ValueType::Int, ValueType::Char) => Some((ValueType::Int, ValueType::Int)),
                (ValueType::Float, ValueType::Int) => Some((ValueType::Float, ValueType::Float)),
                (ValueType::Char, ValueType::Int) => Some((ValueType::Int, ValueType::Int)),
                _ => None
            }
        }
    }

    pub fn func_type(ret_type: &ValueType, params: &Vec<(Token, ValueType)>) -> Self {
        Self::Object(
            ObjectType::Function {
                ret_type: Box::new(ret_type.clone()),
                params: params.iter()
                    .map(|(_, tp)| tp.clone())
                    .collect::<Vec<ValueType>>()
                    .into_boxed_slice()
            }
        )
    }
}

#[derive(Debug, PartialEq)]
pub enum Object {
    String(String),
    Function(Function),
}

impl Clone for Object {
    fn clone(&self) -> Self {
        match self {
            Self::String(s) => Self::String(s.clone()),
            Self::Function(_) => todo!(),
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

impl Object {
    pub fn add(obj1: &Self, obj2: &Self) -> Option<Value> {
        match (obj1, obj2) {
            (Object::String(s1), Object::String(s2)) => Some(Value::from(s1.to_owned() + s2)),
            _ => None,
        }
    }
}

impl ToString for Object {
    fn to_string(&self) -> String {
        match self {
            Object::String(s) => s.to_owned(),
            Object::Function(Function { name, ret_type,.. }) =>
                format!("<func {}(): {}>", name.to_string(), ret_type)
        }
    }
}
