use crate::values::TypedValue;

pub mod objects {

    use super::*;

    pub fn new_object(args: &[TypedValue]) -> TypedValue {
        TypedValue::Object(args.to_vec().into_boxed_slice().into())
    }
}
