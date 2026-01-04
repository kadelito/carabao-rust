use crate::{expr_ast::{Expr, ExprVisitor}, lexing::Token, typed_values::TypedValue, types::ValueType};

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

fn find_opening_token(expr: &Expr) -> &Token {
    expr.accept(&mut ErrorLocator {})
}

struct ErrorLocator {}

impl<'ast> ExprVisitor<'ast, &'ast Token> for ErrorLocator {
    fn visit_conditional_expr(&mut self,
        condition: &'ast Box<Expr>, _if_true: &'ast Box<Expr>, _if_false: &'ast Box<Expr>, __id: usize) -> &'ast Token {
        condition.accept(self)
    }

    fn visit_boolean_expr(&mut self,
        left: &'ast Box<Expr>, _op: &'ast Token, _right: &'ast Box<Expr>, _id: usize) -> &'ast Token {
        left.accept(self)
    }

    fn visit_binary_expr(&mut self,
        left: &'ast Box<Expr>, _op: &'ast Token, _right: &'ast Box<Expr>, _id: usize) -> &'ast Token {
        left.accept(self)
    }

    fn visit_assign_expr(&mut self,
        assignee: &'ast Box<Expr>, _value: &'ast Box<Expr>, _id: usize) -> &'ast Token {
        assignee.accept(self)
    }

    fn visit_cast_expr(&mut self,
        expr: &'ast Box<Expr>, _new_type: &'ast ValueType, _id: usize) -> &'ast Token {
        expr.accept(self)
    }

    fn visit_unary_expr(&mut self,
        op: &'ast Token, target: &'ast Box<Expr>, prefix: &'ast bool, _id: usize) -> &'ast Token {
        if *prefix {
            op
        } else {
            target.accept(self)
        }
    }

    fn visit_slice_expr(&mut self,
        sequence: &'ast Box<Expr>, _query: &'ast Box<Expr>, _id: usize) -> &'ast Token {
        sequence.accept(self)
    }

    fn visit_call_expr(&mut self,
        callee: &'ast Box<Expr>, _args: &'ast Vec<Expr>, _id: usize) -> &'ast Token {
        callee.accept(self)
    }

    fn visit_get_expr(&mut self,
        obj: &'ast Box<Expr>, _property: &'ast Token, _id: usize) -> &'ast Token {
        obj.accept(self)
    }

    fn visit_list_expr(&mut self,
        items: &'ast Vec<Expr>, _id: usize) -> &'ast Token {
        todo!() // TODO make list include open bracket
    }

    fn visit_variable_expr(&mut self,
        identifier: &'ast Token, _id: usize) -> &'ast Token {
        identifier
    }

    fn visit_literal_expr(&mut self,
        repr: &'ast Token, _val: &'ast TypedValue, _id: usize) -> &'ast Token {
        repr
    }
}