pub mod macros {
    macro_rules! internal_error {
        ($msg: expr $(, $arg: expr )*) => {
            panic!(concat!(
                "Internal compiler error at ", file!(),
                ", line ", line!(), ": ", $msg
            )$(, $arg)*)
        };
    }
    
    pub(crate) use internal_error;
}