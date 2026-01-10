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
    pub fn dummy() -> Self {
        Self::Block { statements: vec![] }
    }

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

    pub fn yield_to<T>(&'me mut self, invader: &mut impl StmtInvader<'vis, T>) -> T {
        match self {
            Self::Struct { name, fields } =>
                invader.invade_struct_stmt(name, fields),
            Self::Function { ret_type, name, params, body } =>
                invader.invade_function_stmt(ret_type, name, params, body),
            Self::Summon { path, alias, id } =>
                invader.invade_summon_stmt(path, alias.as_mut(), *id),
            Self::Var { name, var_type, val } =>
                invader.invade_var_stmt(name, var_type.as_mut(), val.as_deref_mut()),
            Self::Block { statements } =>
                invader.invade_block_stmt(statements),
            Self::Expression { expression } =>
                invader.invade_expression_stmt(expression.as_mut()),
            Self::If { condition, true_branch, false_branch } =>
                invader.invade_if_stmt(condition.as_mut(), true_branch.as_mut(), false_branch.as_deref_mut()),
            Self::While { condition, body } =>
                invader.invade_while_stmt(condition.as_mut(), body.as_mut()),
            Self::For { var, sequence, body } =>
                invader.invade_for_stmt(var, sequence.as_mut(), body.as_mut()),
            Self::Keyword { keyword, arg } =>
                invader.invade_keyword_stmt(keyword, arg.as_deref_mut()),
        }
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

/// A mutating Stmt visitor
pub trait StmtInvader<'ast, T> {
    fn invade_struct_stmt(&mut self,
        name: &'ast mut Token, fields: &'ast mut Vec<(Token, ValueType)>) -> T;
    fn invade_function_stmt(&mut self,
        ret_type: &'ast mut ValueType, name: &'ast mut Token, params: &'ast mut Vec<(Token, ValueType)>, body: &'ast mut Vec<Stmt>) -> T;
    fn invade_summon_stmt(&mut self,
        path: &'ast mut Vec<Token>, alias: Option<&'ast mut Token>, id: usize) -> T;
    fn invade_var_stmt(&mut self,
        name: &'ast mut Token, var_type: Option<&'ast mut ValueType>, val: Option<&'ast mut Expr>) -> T;
    fn invade_block_stmt(&mut self,
        statements: &'ast mut Vec<Stmt>) -> T;
    fn invade_expression_stmt(&mut self,
        expression: &'ast mut Expr) -> T;
    fn invade_if_stmt(&mut self,
        condition: &'ast mut Expr, true_branch: &'ast mut Stmt, false_branch: Option<&'ast mut Stmt>) -> T;
    fn invade_while_stmt(&mut self,
        condition: &'ast mut Expr, body: &'ast mut Stmt) -> T;
    fn invade_for_stmt(&mut self,
        var: &'ast mut Token, sequence: &'ast mut Expr, body: &'ast mut Stmt) -> T;
    fn invade_keyword_stmt(&mut self,
        keyword: &'ast mut Token, arg: Option<&'ast mut Expr>) -> T;
}