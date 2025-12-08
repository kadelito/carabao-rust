use std::fmt::Display;

use crate::{lexing::{Token, TokenType}, values};

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
            Display::fmt(obj_type, f)
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

const TYPE_BYTE_OFFSET: u8 = 0x80;

impl ValueType {
    pub fn from_token(value: TokenType) -> Option<Self> {
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

    pub fn can_convert_type(expected: &ValueType, given: &ValueType) -> bool {
        if *expected == ValueType::Any
            || *given == ValueType::Any 
            || *expected == *given {
            // if given is Any, we delegate type checking to runtime
            true
        } else if matches!(expected, ValueType::Object(_)) 
               || matches!(given, ValueType::Object(_)) {
            // No casting to and from objects
            false
        } else {
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