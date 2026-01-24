use std::rc::Rc;

use crate::standard_library::registry::macros::val_into;
use crate::values::TypedValue;

pub mod ranges {
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
    use super::*;

    pub fn new_list(args: &[TypedValue]) -> TypedValue {
        let elements = Vec::from(args);
        TypedValue::List(elements.into())
    }

    pub fn len(args: &[TypedValue]) -> TypedValue {
        let list = val_into!(&args[0] => List);
        TypedValue::Int(list.get().len() as i64)
    }

    pub fn concat(args: &[TypedValue]) -> TypedValue {
        let l2 = val_into!(&args[0] => List);
        let l1 = val_into!(&args[1] => List);
        let mut new_list = Vec::with_capacity(l1.get().len() + l2.get().len());
        new_list.extend_from_slice(l1.get().as_slice());
        new_list.extend_from_slice(l2.get().as_slice());
        TypedValue::from(new_list)
    }

    pub fn slice(args: &[TypedValue]) -> TypedValue {
        let list = val_into!(&args[0] => List).get();
        let (start, end) = {
            let range = val_into!(&args[1] => Range);
            let (start_val, end_val) = range.as_ref();
            (
                *val_into!(start_val => Int) as usize,
                *val_into!(end_val => Int) as usize,
            )
        };

        let sliced = list[start..end].to_vec(); // idk what actually happens :3
        TypedValue::List(sliced.into())
    }

    pub fn index_get(args: &[TypedValue]) -> TypedValue {
        let list = val_into!(&args[0] => List).get();
        let index = {
            let mut value = *val_into!(&args[1] => Int);
            if value < 0 {
                value += list.len() as i64
            }
            value
        };
        // TODO bounds checking
        list[index as usize].clone()
    }

    pub fn index_set(args: &[TypedValue]) -> TypedValue {
        let list = val_into!(&args[1] => List).get_mut();
        let index = {
            let mut value = *val_into!(&args[2] => Int);
            if value < 0 {
                value += list.len() as i64
            }
            value
        };
        // double clone is unfortunate but whatever
        list[index as usize] = args[0].clone();
        args[0].clone()
    }
}

pub mod strings {
    use std::io::Write;

    use thin_dst::ThinRc;

    use super::*;

    pub fn to_string(args: &[TypedValue]) -> TypedValue {
        let mut buf: Vec<u8> = Vec::new();
        write!(buf, "{}", args[0]).expect("writing to buffer should not fail??");
        let str = String::from_utf8(buf).expect("i dont know how utf-8 works");
        TypedValue::from(str)
    }

    pub fn len(args: &[TypedValue]) -> TypedValue {
        let s = val_into!(&args[0] => String);
        TypedValue::Int(s.slice.len() as i64)
    }

    pub fn concat(args: &[TypedValue]) -> TypedValue {
        let s1 = &(**val_into!(&args[0] => String)).slice;
        let s2 = &(**val_into!(&args[1] => String)).slice;
        TypedValue::String(ThinRc::new((), [s1, s2].concat().into_boxed_slice()))
    }

    pub fn slice(args: &[TypedValue]) -> TypedValue {
        let str = &val_into!(&args[0] => String).slice;
        let (start, end) = {
            let range = val_into!(&args[1] => Range);
            let (start_val, end_val) = range.as_ref();
            (
                *val_into!(start_val => Int) as usize,
                *val_into!(end_val => Int) as usize,
            )
        };
        // !! Slices create a new string
        let sliced = ThinRc::new((), str[start..end].to_vec().into_boxed_slice());
        TypedValue::String(sliced)
    }

    pub fn index(args: &[TypedValue]) -> TypedValue {
        let utf16_char = {
            let str = &val_into!(&args[0] => String).slice;
            let index = *val_into!(&args[1] => Int);
            str[index as usize]
        };
        TypedValue::Char(unsafe { char::from_u32_unchecked(utf16_char as u32) })
    }

    /// Implemented natively for speed
    pub fn split(args: &[TypedValue]) -> TypedValue {
        let str = val_into!(&args[0] => String);
        let delim = *val_into!(&args[1] => Char) as u32;
        let mut list = Vec::new();
        let mut temp = Vec::new();
        for c in &str.slice {
            if *c as u32 == delim {
                list.push(TypedValue::String(ThinRc::new((), temp.into_boxed_slice())));
                temp = Vec::new();
            } else {
                temp.push(*c);
            }
        }
        list.push(TypedValue::String(ThinRc::new((), temp.into_boxed_slice())));
        TypedValue::from(list)
    }

    // -N -> a < b
    //  0 -> a = b
    // +N -> a > b
    // Comparisons are determined by unicode scalar value
    pub fn compare(args: &[TypedValue]) -> TypedValue {
        let a_chars = val_into!(&args[0] => String).slice.iter();
        let b_chars = val_into!(&args[1] => String).slice.iter();
        (a_chars.cmp(b_chars) as i64).into()
    }

    // Helper for strings::compare().
    // Returns -1 if a <= b, 1 otherwise.
    // Assumes a == b has already been checked to avoid checking for it separately.
    fn compare_any<T: PartialOrd>(a: T, b: T) -> i64 {
        if a <= b { -1 } else { 1 }
    }
}
