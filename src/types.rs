use std::{fmt::Display, rc::Rc};

use crate::{lexing::{Token, TokenType}, values::{self, Function, Value}};

#[derive(Debug, PartialEq, Clone)]
pub enum ValueType {
    None,
    Any,
    Int,
    Float,
    Char,
    Bool,
    String,
    /// this is just generics all over again
    Function { ret_type: Box<ValueType>, params: Box<[ValueType]> },
}

#[derive(Debug, PartialEq, Clone)]
pub enum ObjectType {
}

impl Display for ValueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueType::None => write!(f, "none"),
            ValueType::Any => write!(f, "any"),
            ValueType::Int => write!(f, "int"),
            ValueType::Float => write!(f, "float"),
            ValueType::Char => write!(f, "char"),
            ValueType::Bool => write!(f, "bool"),
            ValueType::String => write!(f, "string"),
            ValueType::Function { ret_type, params } =>
                write!(f, "func({1}): {0}",
                    ret_type,
                    params.iter()
                        .map(|t| format!("{t}"))
                        .collect::<Vec<String>>()
                        .join(", ")),
        }
    }
}

const TYPE_BYTE_OFFSET: u8 = 0x80;

impl ValueType {
    pub fn dummy(&self) -> Value {
        match self {
            ValueType::Any => Value::Any(Box::new(Value::None)),
            ValueType::Int => todo!(),
            ValueType::Float => todo!(),
            ValueType::Char => todo!(),
            ValueType::Bool => todo!(),
            ValueType::String => Value::String(Rc::new(String::new())),
            ValueType::Function { ret_type, params } => {
                let func = Function {
                    name: String::new(),
                    params: params.clone(),
                    ret_type: *ret_type.clone(),
                    constants: Box::new([]),
                    code: Box::new([]),
                };
                Value::Function(Rc::new(func))
            },
            ValueType::None => todo!(),
        }
    }

    pub fn from_token(value: TokenType) -> Option<Self> {
        match value {
            TokenType::Any => Some(Self::Any),
            TokenType::Int => Some(Self::Int),
            TokenType::Float => Some(Self::Float),
            TokenType::Char => Some(Self::Char),
            TokenType::Bool => Some(Self::Bool),
            TokenType::String => Some(Self::String),
            TokenType::None => Some(Self::None),
            _ => None,
        }
    }

    pub fn can_convert_type(expected: &ValueType, given: &ValueType) -> bool {
        if *expected == ValueType::Any
            || *given == ValueType::Any 
            || *expected == *given {
            // if given is Any, we delegate type checking to runtime
            true
        } else {
            // Note that we don't allow any conversions between objects (rc values)
            match (given, expected) {
                (ValueType::Int, ValueType::Float) => true,
                (ValueType::Int, ValueType::Bool) => true,
                (ValueType::Char, ValueType::Int) => true,
                (ValueType::Bool, ValueType::Int) => true,
                (ValueType::Bool, ValueType::Float) => true,
                _ => false
            } 
        } 
            
    }

    pub fn coerce_binary(type1: &ValueType, type2: &ValueType) -> Option<(ValueType, ValueType)> {
        if type1 == type2 {
            // Already the same type
            Some((type1.clone(), (type2.clone())))
        } else if [type1, type2].contains(&&ValueType::String) {
            // val1 is a string
            Some((ValueType::String, ValueType::String))
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
        Self::Function {
            ret_type: Box::new(ret_type.clone()),
            params: params.iter()
                .map(|(_, tp)| tp.clone())
                .collect::<Vec<ValueType>>()
                .into_boxed_slice()
        }
}
}