use std::{cell::RefCell, fmt::{Display, write}, rc::Rc};

use crate::{codegen::{LineRLE, OpCode}, lexing::{Token, TokenType}, values::{Function, TypedValue}};

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
    Function(Box<FunctionType>),
    Object(Box<ObjectType>),
    List(Box<ValueType>),
}

#[derive(Debug, PartialEq, Clone)]
pub struct FunctionType {
    pub ret_type: ValueType,
    pub params: Box<[ValueType]>
}

#[derive(Debug, PartialEq, Clone)]
pub struct ObjectType {
    
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
            ValueType::Function( func )=>
                write!(f, "func({1}): {0}",
                    func.ret_type,
                    func.params.iter()
                        .map(|t| format!("{t}"))
                        .collect::<Vec<String>>()
                        .join(", ")),
            ValueType::List(t) => write!(f, "{}[]", t.to_string()),
            ValueType::Object(object_type) => todo!(),
        }
    }
}

const TYPE_BYTE_OFFSET: u8 = 0x80;

impl ValueType {
    pub fn dummy(&self) -> TypedValue {
        // TODO document this
        match self {
            ValueType::Any => TypedValue::Any(Box::new(TypedValue::None)),
            ValueType::Int => TypedValue::Int(0),
            ValueType::Float => TypedValue::Float(0.0),
            ValueType::Char => TypedValue::Char('\0'),
            ValueType::Bool => TypedValue::Bool(false),
            ValueType::String => TypedValue::String(Rc::new(String::new())),
            ValueType::Function(func) => {
                let func = Function {
                    name: String::new(),
                    params: func.params.clone(),
                    ret_type: func.ret_type.clone(),
                    constants: Box::new([func.ret_type.dummy()]),
                    code: vec![
                        // return dummy value from constants
                        OpCode::Constant.into(), 0,
                        OpCode::Return.into(),
                    ].into_boxed_slice(),
                    lines: Box::new([LineRLE { line: 0, count: 3 }]),
                };
                TypedValue::Function(Rc::new(func))
            },
            ValueType::List(_)
            => TypedValue::List(Rc::new(RefCell::new(Vec::new()))),
            ValueType::Object(object_type) => todo!(),
            ValueType::None => TypedValue::None,
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

    /// Returns whether an explicit cast works from `given` to `expected`.
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

    pub fn coerce_binary(type1: &ValueType, op: TokenType, type2: &ValueType) -> Option<(ValueType, ValueType)> {
        if type1 == type2 {
            // Already the same type
            Some((type1.clone(), (type2.clone())))
        } else if op == TokenType::Plus && [type1, type2].contains(&&ValueType::String) {
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
        Self::Function(FunctionType {
            ret_type: ret_type.clone(),
            params: params.iter()
                .map(|(_, tp)| tp.clone())
                .collect::<Vec<ValueType>>()
                .into_boxed_slice()
        }.into())
}
}