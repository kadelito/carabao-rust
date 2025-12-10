use crate::lexing::Token;
use crate::types::*;
use crate::expr_ast::Expr;

#[derive(Debug)]
pub enum Stmt {
    // TODO the commented-out ones
    Function { ret_type: ValueType, name: Token, params: Vec<(Token, ValueType)>, body: Vec<Stmt>, id: usize },
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
        Self::Block { statements: Vec::new() }
    }

    pub fn accept<T>(&'me self, visitor: &mut impl StmtVisitor<'vis, T>) -> T {
        match self {
            Self::Function { ret_type, name, params, body, id } =>
                visitor.visit_function_stmt(ret_type, name, params, body, *id),
            Self::Summon { path, alias, id } =>
                visitor.visit_summon_stmt(path, alias, *id),
            Self::Var { name, var_type, val } =>
                visitor.visit_var_stmt(name, var_type, val),
            Self::Block { statements } =>
                visitor.visit_block_stmt(statements),
            Self::Expression { expression } =>
                visitor.visit_expression_stmt(expression),
            Self::If { condition, true_branch, false_branch } =>
                visitor.visit_if_stmt(condition, true_branch, false_branch),
            Self::While { condition, body } =>
                visitor.visit_while_stmt(condition, body),
            Self::For { var, sequence, body } =>
                visitor.visit_for_stmt(var, sequence, body),
            Self::Keyword { keyword, arg } =>
                visitor.visit_keyword_stmt(keyword, arg),
        }
    }
}

pub trait StmtVisitor<'ast, T> {
    fn visit_function_stmt(&mut self,
        ret_type: &'ast ValueType, name: &'ast Token, params: &'ast Vec<(Token, ValueType)>, body: &'ast Vec<Stmt>, id: usize) -> T;
    fn visit_summon_stmt(&mut self,
        path: &'ast Vec<Token>, alias: &'ast Option<Token>, id: usize) -> T;
    fn visit_var_stmt(&mut self,
        name: &'ast Token, var_type: &'ast Option<ValueType>, val: &'ast Option<Box<Expr>>) -> T;
    fn visit_block_stmt(&mut self,
        statements: &'ast Vec<Stmt>) -> T;
    fn visit_expression_stmt(&mut self,
        expression: &'ast Box<Expr>) -> T;
    fn visit_if_stmt(&mut self,
        condition: &'ast Box<Expr>, true_branch: &'ast Box<Stmt>, false_branch: &'ast Option<Box<Stmt>>) -> T;
    fn visit_while_stmt(&mut self,
        condition: &'ast Box<Expr>, body: &'ast Box<Stmt>) -> T;
    fn visit_for_stmt(&mut self,
        var: &'ast Token, sequence: &'ast Box<Expr>, body: &'ast Box<Stmt>) -> T;
    fn visit_keyword_stmt(&mut self,
        keyword: &'ast Token, arg: &'ast Option<Box<Expr>>) -> T;
}