use crate::lexing::Token;
use crate::types::*;
use crate::values::*;

#[derive(Debug)]
pub enum Expr {
    // TODO the commented-out ones
    Conditional { condition: Box<Expr>, if_true: Box<Expr>, if_false: Box<Expr>, id: usize },
    Boolean { left: Box<Expr>, op: Token, right: Box<Expr>, id: usize },
    Binary { left: Box<Expr>, op: Token, right: Box<Expr>, id: usize },
    Assign { assignee: Box<Expr>, value: Box<Expr>, id: usize },
    Cast { expr: Box<Expr>, new_type: ValueType, id: usize },
    Unary { op: Token, target: Box<Expr>, prefix: bool, id: usize },
    Slice { sequence: Box<Expr>, query: Box<Expr>, id: usize },
    Call { callee: Box<Expr>, args: Vec<Expr>, id: usize },
    Method { obj: Box<Expr>, method: Token, args: Vec<Expr>, id: usize },
    Get { obj: Box<Expr>, property: Token, id: usize },
    List { items: Vec<Expr>, id: usize },
    Variable { identifier: Token, id: usize },
    Literal { repr: Token, val: Value, id: usize },
}

impl<'me, 'vis> Expr where 'me: 'vis {
    pub fn dummy() -> Self {
        Self::List { items: Vec::new(), id: 0 }
    }

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
            Self::Method { id, .. } => *id,
            Self::Get { id, .. } => *id,
            Self::List { id, .. } => *id,
            Self::Variable { id, .. } => *id,
            Self::Literal { id, .. } => *id,
        }
    }
    pub fn accept<T>(&'me self, visitor: &mut impl ExprVisitor<'vis, T>) -> T {
        match self {
            Self::Conditional { condition, if_true, if_false, id } =>
                visitor.visit_conditional_expr(condition, if_true, if_false, *id),
            Self::Boolean { left, op, right, id } =>
                visitor.visit_boolean_expr(left, op, right, *id),
            Self::Binary { left, op, right, id } =>
                visitor.visit_binary_expr(left, op, right, *id),
            Self::Assign { assignee, value, id } =>
                visitor.visit_assign_expr(assignee, value, *id),
            Self::Cast { expr, new_type, id } =>
                visitor.visit_cast_expr(expr, new_type, *id),
            Self::Unary { op, target, prefix, id } =>
                visitor.visit_unary_expr(op, target, prefix, *id),
            Self::Slice { sequence, query, id } =>
                visitor.visit_slice_expr(sequence, query, *id),
            Self::Call { callee, args, id } =>
                visitor.visit_call_expr(callee, args, *id),
            Self::Method { obj, method, args, id } =>
                visitor.visit_method_expr(obj, method, args, *id),
            Self::Get { obj, property, id } =>
                visitor.visit_get_expr(obj, property, *id),
            Self::List { items, id } =>
                visitor.visit_list_expr(items, *id),
            Self::Variable { identifier, id } =>
                visitor.visit_variable_expr(identifier, *id),
            Self::Literal { repr, val, id } =>
                visitor.visit_literal_expr(repr, val, *id),
        }
    }
}

pub trait ExprVisitor<'ast, T> {
    fn visit_conditional_expr(&mut self,
        condition: &'ast Box<Expr>, if_true: &'ast Box<Expr>, if_false: &'ast Box<Expr>, id: usize) -> T;
    fn visit_boolean_expr(&mut self,
        left: &'ast Box<Expr>, op: &'ast Token, right: &'ast Box<Expr>, id: usize) -> T;
    fn visit_binary_expr(&mut self,
        left: &'ast Box<Expr>, op: &'ast Token, right: &'ast Box<Expr>, id: usize) -> T;
    fn visit_assign_expr(&mut self,
        assignee: &'ast Box<Expr>, value: &'ast Box<Expr>, id: usize) -> T;
    fn visit_cast_expr(&mut self,
        expr: &'ast Box<Expr>, new_type: &'ast ValueType, id: usize) -> T;
    fn visit_unary_expr(&mut self,
        op: &'ast Token, target: &'ast Box<Expr>, prefix: &'ast bool, id: usize) -> T;
    fn visit_slice_expr(&mut self,
        sequence: &'ast Box<Expr>, query: &'ast Box<Expr>, id: usize) -> T;
    fn visit_call_expr(&mut self,
        callee: &'ast Box<Expr>, args: &'ast Vec<Expr>, id: usize) -> T;
    fn visit_method_expr(&mut self,
        obj: &'ast Box<Expr>, method: &'ast Token, args: &'ast Vec<Expr>, id: usize) -> T;
    fn visit_get_expr(&mut self,
        obj: &'ast Box<Expr>, property: &'ast Token, id: usize) -> T;
    fn visit_list_expr(&mut self,
        items: &'ast Vec<Expr>, id: usize) -> T;
    fn visit_variable_expr(&mut self,
        identifier: &'ast Token, id: usize) -> T;
    fn visit_literal_expr(&mut self,
        repr: &'ast Token, val: &'ast Value, id: usize) -> T;
}