use std::{cell::RefCell, rc::Rc};

use crate::{values::TypedValue, errors::macros::internal_error};

pub mod ranges {

    use crate::standard_library::registry::macros::val_into;

    use super::*;

    pub fn new_range(args: &[TypedValue]) -> TypedValue {
        let start = args[0].clone();
        let end = args[1].clone();
        TypedValue::Range(Rc::new((start, end)))
    }

    pub fn range_get_start(args: &[TypedValue]) -> TypedValue {
        let range = val_into!(&args[0] => Range);
        range.0.clone()
    }

    pub fn range_get_end(args: &[TypedValue]) -> TypedValue {
        let range = val_into!(&args[0] => Range);
        range.1.clone()
    }
}

pub mod lists {
    use crate::standard_library::registry::macros::val_into;

    use super::*;

    pub fn new_list(args: &[TypedValue]) -> TypedValue {
        let elements = Vec::from(args);
        TypedValue::List(Rc::new(RefCell::new(elements)))
    }

    pub fn list_len(args: &[TypedValue]) -> TypedValue {
        let list = val_into!(&args[0] => List);
        TypedValue::Int(list.borrow().len() as i64)
    }

    pub fn list_concat(args: &[TypedValue]) -> TypedValue {
        let l2 = val_into!(&args[0] => List);
        let l1 = val_into!(&args[1] => List);
        let mut new_list = Vec::with_capacity(l1.borrow().len() + l2.borrow().len());
        new_list.extend_from_slice(l1.borrow().as_slice());
        new_list.extend_from_slice(l2.borrow().as_slice());
        TypedValue::from(new_list)
    }
}