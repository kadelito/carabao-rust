use std::{cell::RefCell, rc::Rc};

use crate::values::TypedValue;

macro_rules! get_from_any {
    ($variant: ident($any_val: expr)) => {{
        // TODO replace with a union access
        let TypedValue::Any(val) = $any_val else { panic!() };
        let TypedValue::$variant(val) = val.as_ref() else { panic!() };
        val
    }};
}

mod ranges {
    use super::*;

    pub fn new_range(args: &[TypedValue]) -> TypedValue {
        let start = args[0].clone();
        let end = args[1].clone();
        TypedValue::Range(Rc::new((start, end)))
    }

    pub fn range_get_start(args: &[TypedValue]) -> TypedValue {
        let range = get_from_any!(Range(&args[0]));
        range.0.clone()
    }

    pub fn range_get_end(args: &[TypedValue]) -> TypedValue {
        let range = get_from_any!(Range(&args[0]));
        range.0.clone()
    }
}

pub fn list_len(args: &[TypedValue]) -> TypedValue {
    let list = get_from_any!(List(&args[0]));
    TypedValue::Int(list.borrow().len() as i64)
}

pub fn list_concat(args: &[TypedValue]) -> TypedValue {
    let l1 = get_from_any!(List(&args[0]));
    let l2 = get_from_any!(List(&args[1]));
    let mut new_list = Vec::with_capacity(l1.borrow().len() + l2.borrow().len());
    new_list.extend_from_slice(l1.borrow().as_slice());
    new_list.extend_from_slice(l2.borrow().as_slice());
    TypedValue::from(new_list)
}