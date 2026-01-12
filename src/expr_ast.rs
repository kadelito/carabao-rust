use std::usize;

use crate::lexing::Token;
use crate::types::*;
use crate::values::*;

#[derive(Debug)]
pub enum Expr {
    Conditional { condition: Box<Expr>, if_true: Box<Expr>, if_false: Box<Expr>, id: usize },
    Boolean { left: Box<Expr>, op: Token, right: Box<Expr>, id: usize },
    Binary { left: Box<Expr>, op: Token, right: Box<Expr>, id: usize },
    Assign { assignee: Box<Expr>, op: Token, value: Box<Expr>, id: usize },
    Cast { expr: Box<Expr>, new_type: ValueType, id: usize },
    Unary { op: Token, target: Box<Expr>, prefix: bool, id: usize },
    Slice { sequence: Box<Expr>, query: Box<Expr>, id: usize },
    Call { callee: Box<Expr>, args: Vec<Expr>, id: usize },
    Get { obj: Box<Expr>, property: Token, id: usize },
    List { items: Vec<Expr>, id: usize },
    Variable { identifier: Token, id: usize },
    Literal { repr: Token, val: TypedValue, id: usize },
}

impl<'me, 'vis> Expr where 'me: 'vis {
    pub fn id(&self) -> usize {
        match self {
            Self::Conditional { id, .. } => *id,
            Self::Boolean { id, .. } => *id,
            Self::Binary { id, .. } => *id,
            Self::Assign { id, .. } => *id,
            Self::Cast { id, .. } => *id,
            Self::Unary { id, .. } => *id,
            Self::Slice { id, .. } => *id,
            Self::Call { id, .. } => *id,
            Self::Get { id, .. } => *id,
            Self::List { id, .. } => *id,
            Self::Variable { id, .. } => *id,
            Self::Literal { id, .. } => *id,
        }
    }

    pub fn accept<T>(&'me self, visitor: &mut impl ExprVisitor<'vis, T>) -> T {
        match self {
            Self::Conditional { condition, if_true, if_false, id } =>
                visitor.visit_conditional_expr(condition.as_ref(), if_true.as_ref(), if_false.as_ref(), *id),
            Self::Boolean { left, op, right, id } =>
                visitor.visit_boolean_expr(left.as_ref(), op, right.as_ref(), *id),
            Self::Binary { left, op, right, id } =>
                visitor.visit_binary_expr(left.as_ref(), op, right.as_ref(), *id),
            Self::Assign { assignee, op, value, id } =>
                visitor.visit_assign_expr(assignee.as_ref(), op, value.as_ref(), *id),
            Self::Cast { expr, new_type, id } =>
                visitor.visit_cast_expr(expr.as_ref(), new_type, *id),
            Self::Unary { op, target, prefix, id } =>
                visitor.visit_unary_expr(op, target.as_ref(), prefix, *id),
            Self::Slice { sequence, query, id } =>
                visitor.visit_slice_expr(sequence.as_ref(), query.as_ref(), *id),
            Self::Call { callee, args, id } =>
                visitor.visit_call_expr(callee.as_ref(), args, *id),
            Self::Get { obj, property, id } =>
                visitor.visit_get_expr(obj.as_ref(), property, *id),
            Self::List { items, id } =>
                visitor.visit_list_expr(items, *id),
            Self::Variable { identifier, id } =>
                visitor.visit_variable_expr(identifier, *id),
            Self::Literal { repr, val, id } =>
                visitor.visit_literal_expr(repr, val, *id),
        }
    }

    pub fn yield_to<T>(&'me mut self, invader: &mut impl ExprInvader<'vis, T>) -> T {
        match self {
            Self::Conditional { condition, if_true, if_false, id } =>
                invader.invade_conditional_expr(condition.as_mut(), if_true.as_mut(), if_false.as_mut(), *id),
            Self::Boolean { left, op, right, id } =>
                invader.invade_boolean_expr(left.as_mut(), op, right.as_mut(), *id),
            Self::Binary { left, op, right, id } =>
                invader.invade_binary_expr(left.as_mut(), op, right.as_mut(), *id),
            Self::Assign { assignee, op, value, id } =>
                invader.invade_assign_expr(assignee.as_mut(), op, value.as_mut(), *id),
            Self::Cast { expr, new_type, id } =>
                invader.invade_cast_expr(expr.as_mut(), new_type, *id),
            Self::Unary { op, target, prefix, id } =>
                invader.invade_unary_expr(op, target.as_mut(), prefix, *id),
            Self::Slice { sequence, query, id } =>
                invader.invade_slice_expr(sequence.as_mut(), query.as_mut(), *id),
            Self::Call { callee, args, id } =>
                invader.invade_call_expr(callee.as_mut(), args, *id),
            Self::Get { obj, property, id } =>
                invader.invade_get_expr(obj.as_mut(), property, *id),
            Self::List { items, id } =>
                invader.invade_list_expr(items, *id),
            Self::Variable { identifier, id } =>
                invader.invade_variable_expr(identifier, *id),
            Self::Literal { repr, val, id } =>
                invader.invade_literal_expr(repr, val, *id),
        }
    }
}

impl Default for Expr {
    /// Returns a 'dummy' value when an Expr is expected but some error occured.
    /// It is expected that this Expr never actually gets examined.
    fn default() -> Self {
        Self::List { items: vec![], id: usize::MAX }
    }
}

pub trait ExprVisitor<'ast, T> {
    fn visit_conditional_expr(&mut self,
        condition: &'ast Expr, if_true: &'ast Expr, if_false: &'ast Expr, id: usize) -> T;
    fn visit_boolean_expr(&mut self,
        left: &'ast Expr, op: &'ast Token, right: &'ast Expr, id: usize) -> T;
    fn visit_binary_expr(&mut self,
        left: &'ast Expr, op: &'ast Token, right: &'ast Expr, id: usize) -> T;
    fn visit_assign_expr(&mut self,
        assignee: &'ast Expr, op: &'ast Token, value: &'ast Expr, id: usize) -> T;
    fn visit_cast_expr(&mut self,
        expr: &'ast Expr, new_type: &'ast ValueType, id: usize) -> T;
    fn visit_unary_expr(&mut self,
        op: &'ast Token, target: &'ast Expr, prefix: &'ast bool, id: usize) -> T;
    fn visit_slice_expr(&mut self,
        sequence: &'ast Expr, query: &'ast Expr, id: usize) -> T;
    fn visit_call_expr(&mut self,
        callee: &'ast Expr, args: &'ast Vec<Expr>, id: usize) -> T;
    fn visit_get_expr(&mut self,
        obj: &'ast Expr, property: &'ast Token, id: usize) -> T;
    fn visit_list_expr(&mut self,
        items: &'ast Vec<Expr>, id: usize) -> T;
    fn visit_variable_expr(&mut self,
        identifier: &'ast Token, id: usize) -> T;
    fn visit_literal_expr(&mut self,
        repr: &'ast Token, val: &'ast TypedValue, id: usize) -> T;
}

/// A mutating Expr visitor
pub trait ExprInvader<'ast, T> {
    fn invade_conditional_expr(&mut self,
        condition: &'ast mut Expr, if_true: &'ast mut Expr, if_false: &'ast mut Expr, id: usize) -> T;
    fn invade_boolean_expr(&mut self,
        left: &'ast mut Expr, op: &'ast mut Token, right: &'ast mut Expr, id: usize) -> T;
    fn invade_binary_expr(&mut self,
        left: &'ast mut Expr, op: &'ast mut Token, right: &'ast mut Expr, id: usize) -> T;
    fn invade_assign_expr(&mut self,
        assignee: &'ast mut Expr, op: &'ast mut Token, value: &'ast mut Expr, id: usize) -> T;
    fn invade_cast_expr(&mut self,
        expr: &'ast mut Expr, new_type: &'ast mut ValueType, id: usize) -> T;
    fn invade_unary_expr(&mut self,
        op: &'ast mut Token, target: &'ast mut Expr, prefix: &'ast mut bool, id: usize) -> T;
    fn invade_slice_expr(&mut self,
        sequence: &'ast mut Expr, query: &'ast mut Expr, id: usize) -> T;
    fn invade_call_expr(&mut self,
        callee: &'ast mut Expr, args: &'ast mut Vec<Expr>, id: usize) -> T;
    fn invade_get_expr(&mut self,
        obj: &'ast mut Expr, property: &'ast mut Token, id: usize) -> T;
    fn invade_list_expr(&mut self,
        items: &'ast mut Vec<Expr>, id: usize) -> T;
    fn invade_variable_expr(&mut self,
        identifier: &'ast mut Token, id: usize) -> T;
    fn invade_literal_expr(&mut self,
        repr: &'ast mut Token, val: &'ast mut TypedValue, id: usize) -> T;
}