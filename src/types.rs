use std::{cell::RefCell, fmt::Display, rc::Rc};

use crate::{
    codegen::{LineRLE, OpCode},
    errors::macros::internal_error,
    lexing::{Token, TokenType},
    values::{TypedFunction, TypedNativeFunction, TypedValue},
};

#[derive(Debug, Clone)]
pub enum ValueType {
    Unchecked,
    None,
    Any,
    Int,
    Float,
    Char,
    Bool,
    String,
    // my god they're generic
    Range(Box<ValueType>),
    Function(Box<FunctionType>),
    UserType(Token),
    Object(Box<ObjectType>),
    List(Box<ValueType>),
}

impl PartialEq for ValueType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Range(l0), Self::Range(r0)) => l0 == r0,
            (Self::Function(l0), Self::Function(r0)) => l0 == r0,
            (Self::UserType(l0), Self::UserType(r0)) => l0.lexeme() == r0.lexeme(),
            (Self::Object(l0), Self::Object(r0)) => l0 == r0,
            (Self::List(l0), Self::List(r0)) => l0 == r0,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct FunctionType {
    pub ret_type: ValueType,
    pub native: bool,
    pub params: Box<[ValueType]>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ObjectType {
    pub fields: Box<[(String, ValueType)]>,
}

impl Display for ValueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueType::Unchecked => internal_error!("Unchecked types should not be displayed"),
            ValueType::None => write!(f, "none"),
            ValueType::Any => write!(f, "any"),
            ValueType::Int => write!(f, "int"),
            ValueType::Float => write!(f, "float"),
            ValueType::Char => write!(f, "char"),
            ValueType::Bool => write!(f, "bool"),
            ValueType::String => write!(f, "string"),
            ValueType::Range(t) => write!(f, "{0}..", t),
            ValueType::Function(func) => write!(f,
                "func({1}): {0}",
                func.ret_type,
                func.params
                    .iter()
                    .map(|t| format!("{t}"))
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            ValueType::List(t) => write!(f, "{}[]", t.to_string()),
            ValueType::Object(obj) => write!(f,
                "{{{}}}",
                obj.fields
                    .iter()
                    .map(|(s, t)| format!("{t} {s}"))
                    .collect::<Vec<String>>()
                    .join("; ")
            ),
            ValueType::UserType(s) => write!(f, "{s}"),
        }
    }
}

impl From<&TypedNativeFunction> for ValueType {
    fn from(value: &TypedNativeFunction) -> Self {
        let TypedNativeFunction {
            params, ret_type, ..
        } = value;
        ValueType::Function(Box::new(FunctionType {
            ret_type: ret_type.clone(),
            params: params.to_vec().into_boxed_slice(),
            native: true,
        }))
    }
}

impl From<FunctionType> for ValueType {
    fn from(value: FunctionType) -> Self {
        Self::Function(value.into())
    }
}

impl ValueType {
    pub fn dummy(&self) -> TypedValue {
        // TODO dummy documentation
        match self {
            ValueType::Unchecked => TypedValue::None,
            ValueType::None => TypedValue::None,
            ValueType::Any => TypedValue::Any(Box::new(TypedValue::None)),
            ValueType::Int => TypedValue::Int(0),
            ValueType::Float => TypedValue::Float(0.0),
            ValueType::Char => TypedValue::Char('\0'),
            ValueType::Bool => TypedValue::Bool(false),
            ValueType::String => TypedValue::String(Rc::new([].into())),
            ValueType::Range(t) => TypedValue::Range(Rc::new((t.dummy(), t.dummy()))),
            ValueType::Function(func) => {
                let func = TypedFunction {
                    name: String::new().into_boxed_str(),
                    params: func.params.clone(),
                    ret_type: func.ret_type.clone(),
                    constants: Box::new([func.ret_type.dummy()]),
                    code: vec![
                        // return dummy value from constants
                        OpCode::Constant.into(),
                        0,
                        OpCode::Return.into(),
                    ]
                    .into_boxed_slice(),
                    lines: Box::new([LineRLE { line: 0, count: 3 }]),
                };
                TypedValue::Function(Rc::new(func))
            }
            // empty list
            ValueType::List(_) => TypedValue::List(vec![].into()),
            ValueType::Object(obj) => {
                let ObjectType { fields } = obj.as_ref();
                let fields: Vec<_> = fields.clone().into_iter().map(|(_, kind)| kind.dummy()).collect();
                TypedValue::Object(fields.into_boxed_slice().into())
            },
            ValueType::UserType(_) => todo!(),
        }
    }

    pub fn from_token(token: Token) -> Option<Self> {
        match token.kind() {
            TokenType::Any => Some(Self::Any),
            TokenType::Int => Some(Self::Int),
            TokenType::Float => Some(Self::Float),
            TokenType::Char => Some(Self::Char),
            TokenType::Bool => Some(Self::Bool),
            TokenType::String => Some(Self::String),
            TokenType::None => Some(Self::None),
            TokenType::Identifier => Some(Self::UserType(token)),
            _ => None,
        }
    }

    /// Returns whether an explicit cast works from `given` to `expected`.
    pub fn can_convert_type(expected: &ValueType, given: &ValueType) -> bool {
        if *given == ValueType::Unchecked {
            true
        } else if *expected == ValueType::Any || *given == ValueType::Any || *expected == *given {
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
                _ => false,
            }
        }
    }

    pub fn coerce_binary(
        type1: &ValueType,
        is_adding: bool,
        type2: &ValueType,
    ) -> Option<ValueType> {
        if type1 == type2 {
            // Already the same type
            Some(type1.clone())
        } else if is_adding && [type1, type2].contains(&&ValueType::String) {
            // val1 is a string
            Some(ValueType::String)
        } else {
            match (type1, type2) {
                (ValueType::Int, ValueType::Float) => Some(ValueType::Float),
                (ValueType::Int, ValueType::Char) => Some(ValueType::Int),
                (ValueType::Float, ValueType::Int) => Some(ValueType::Float),
                (ValueType::Char, ValueType::Int) => Some(ValueType::Int),
                _ => None,
            }
        }
    }

    pub fn func_type(ret_type: &ValueType, params: &Vec<(Token, ValueType)>) -> Self {
        Self::Function(
            FunctionType {
                ret_type: ret_type.clone(),
                params: params
                    .iter()
                    .map(|(_, tp)| tp.clone())
                    .collect::<Vec<ValueType>>()
                    .into_boxed_slice(),
                native: false,
            }
            .into(),
        )
    }
}
