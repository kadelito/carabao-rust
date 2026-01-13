use crate::lexing::Token;
use crate::types::*;
use crate::expr_ast::Expr;

#[derive(Debug)]
pub enum Stmt {
    Struct { name: Token, fields: Vec<(Token, ValueType)> },
    Function { ret_type: ValueType, name: Token, params: Vec<(Token, ValueType)>, body: Vec<Stmt> },
    Summon { path: Vec<Token>, alias: Option<Token>, id: usize },
    Var { name: Token, var_type: Option<ValueType>, val: Option<Box<Expr>> },
    Block { statements: Vec<Stmt> },
    Expression { expression: Box<Expr> },
    If { condition: Box<Expr>, true_branch: Box<Stmt>, false_branch: Option<Box<Stmt>> },
    While { condition: Box<Expr>, body: Box<Stmt> },
    For { var: Token, sequence: Box<Expr>, body: Box<Stmt> },
    Keyword { keyword: Token, arg: Option<Box<Expr>> },
}

impl<'me, 'vis> Stmt where 'me: 'vis {
    pub fn accept<T>(&'me self, visitor: &mut impl StmtVisitor<'vis, T>) -> T {
        match self {
            Self::Struct { name, fields } =>
                visitor.visit_struct_stmt(name, fields),
            Self::Function { ret_type, name, params, body } =>
                visitor.visit_function_stmt(ret_type, name, params, body),
            Self::Summon { path, alias, id } =>
                visitor.visit_summon_stmt(path, alias.as_ref(), *id),
            Self::Var { name, var_type, val } =>
                visitor.visit_var_stmt(name, var_type.as_ref(), val.as_deref()),
            Self::Block { statements } =>
                visitor.visit_block_stmt(statements),
            Self::Expression { expression } =>
                visitor.visit_expression_stmt(expression.as_ref()),
            Self::If { condition, true_branch, false_branch } =>
                visitor.visit_if_stmt(condition.as_ref(), true_branch.as_ref(), false_branch.as_deref()),
            Self::While { condition, body } =>
                visitor.visit_while_stmt(condition.as_ref(), body.as_ref()),
            Self::For { var, sequence, body } =>
                visitor.visit_for_stmt(var, sequence.as_ref(), body.as_ref()),
            Self::Keyword { keyword, arg } =>
                visitor.visit_keyword_stmt(keyword, arg.as_deref()),
        }
    }
}

impl Default for Stmt {
    /// Returns a dummy value when a Stmt is expected but some error occured.
    /// It is expected that this Stmt never actually gets examined.
    fn default() -> Self {
        Self::Block { statements: vec![] }
    }
}

pub trait StmtVisitor<'ast, T> {
    fn visit_struct_stmt(&mut self,
        name: &'ast Token, fields: &'ast Vec<(Token, ValueType)>) -> T;
    fn visit_function_stmt(&mut self,
        ret_type: &'ast ValueType, name: &'ast Token, params: &'ast Vec<(Token, ValueType)>, body: &'ast Vec<Stmt>) -> T;
    fn visit_summon_stmt(&mut self,
        path: &'ast Vec<Token>, alias: Option<&'ast Token>, id: usize) -> T;
    fn visit_var_stmt(&mut self,
        name: &'ast Token, var_type: Option<&'ast ValueType>, val: Option<&'ast Expr>) -> T;
    fn visit_block_stmt(&mut self,
        statements: &'ast Vec<Stmt>) -> T;
    fn visit_expression_stmt(&mut self,
        expression: &'ast Expr) -> T;
    fn visit_if_stmt(&mut self,
        condition: &'ast Expr, true_branch: &'ast Stmt, false_branch: Option<&'ast Stmt>) -> T;
    fn visit_while_stmt(&mut self,
        condition: &'ast Expr, body: &'ast Stmt) -> T;
    fn visit_for_stmt(&mut self,
        var: &'ast Token, sequence: &'ast Expr, body: &'ast Stmt) -> T;
    fn visit_keyword_stmt(&mut self,
        keyword: &'ast Token, arg: Option<&'ast Expr>) -> T;
}