use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::expr_ast::*;
use crate::lexing::{Token, TokenType};
use crate::stmt_ast::{Stmt, StmtVisitor};
use crate::values::*;

pub fn analyze(program: &Vec<Stmt>) -> Option<AnalysisResult> {
    // Initialize stuff
    let globals = Rc::new(RefCell::new(Globals::new()));
    let mut resolver = FunctionResolver::new(true);
    resolver.globals = globals;

    resolver.resolve(program)
}

pub struct AnalysisResult {
    // map of variable/assign expr id -> location
    pub bindings: HashMap<usize, Binding>,
    pub bin_types: HashMap<usize, ValueType>,
    pub expr_types: HashMap<usize, ValueType>,
}

impl AnalysisResult {
    fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            bin_types: HashMap::new(),
            expr_types: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Binding {
    Stack(usize),
    Globals(usize),
}

#[derive(Debug, Clone, Copy)]
pub enum UsageError {
    TypeError,
    UndefinedIdent,
    InvalidAssign,
    ParamMismatch,
    MustInitGlobal,
    InvalidLoopControl,
    IncompatibleTypes,
    InvalidOperator,
    TooManyConstants,
}

struct FunctionResolver<'ast> {
    ret_type: ValueType,
    /// this will be the constant pool i think
    value_count: usize,
    /// jlox reference
    local_bindings: Vec<VarData>,
    depth: usize,
    is_global: bool,
    loop_depth: usize,
    function_backlog: HashMap<String, TempFunction<'ast>>,

    globals: Rc<RefCell<Globals>>,
}

struct TempFunction<'ast> {
    ret_type: &'ast ValueType,
    params: &'ast Vec<(Token, ValueType)>,
    body: &'ast Vec<Stmt>,
}

struct Globals {
    had_error: bool,
    bindings: Vec<VarData>,
    final_data: AnalysisResult,
}

impl Globals {
    fn new() -> Self {
        Self {
            bindings: Vec::new(),
            had_error: false,
            final_data: AnalysisResult::new(),
        }
    }
}

#[derive(Clone)]
struct VarData {
    pub name: String,
    /// if None, declared but not defined
    pub val_type: Option<ValueType>,
    /// how many blocks into the function
    pub depth: usize,
}

impl<'ast> FunctionResolver<'ast> {
    pub fn new(is_global: bool) -> Self {
        Self {
            ret_type: ValueType::None,
            value_count: 0,
            local_bindings: Vec::new(),
            depth: 0,
            is_global,
            loop_depth: 0,
            function_backlog: HashMap::new(),
            // begins pointing to some dummy value
            globals: Rc::new(RefCell::new(Globals::new())),
        }
    }

    fn resolve_stmt(&mut self, stmt: &'ast Stmt) {
        stmt.accept(self)
    }

    fn resolve_expr(&mut self, expr: &Expr) -> ValueType {
        let expr_type = expr.accept(self);
        self.globals.borrow_mut().final_data.expr_types.insert(expr.id(), expr_type.clone());
        expr_type
    }

    pub fn resolve(&mut self, stmts: &'ast Vec<Stmt>) -> Option<AnalysisResult> {
        for stmt in stmts {
            self.resolve_stmt(stmt);
        }
        if self.globals.borrow().had_error {
            None
        } else {
            let data = std::mem::replace(
                &mut self.globals.borrow_mut().final_data,
                AnalysisResult::new(),
            );
            Some(data)
        }
    }

    fn find_var(&self, ident: &String) -> Option<Binding> {
        // Look in locals first
        for (i, data) in self.local_bindings.iter().enumerate().rev() {
            // Reversed because stack
            if *data.name == *ident {
                return Some(Binding::Stack(i));
            }
        }
        // Look in globals next
        for (i, data) in self.globals.borrow().bindings.iter().enumerate() {
            if *data.name == *ident {
                return Some(Binding::Globals(i));
            }
        }
        // Not in either
        None
    }

    /// Returns the info and location of a variable.
    /// If it hasn't been initialized, returns None.
    fn get_var(&self, ident: &String) -> Option<(Binding, VarData)> {
        let binding = self.find_var(ident)?;
        match binding {
            Binding::Stack(index) => Some((binding, self.local_bindings[index].clone())),
            Binding::Globals(index) => {
                Some((binding, self.globals.borrow().bindings[index].clone()))
            }
        }
    }

    fn declare_global(&mut self, name: String, val_type: ValueType) {
        let globals = &mut self.globals.borrow_mut().bindings;
        for i in 0..globals.len() {
            if *globals[i].name == name {
                // Redeclared
                globals[i] = VarData {
                    name,
                    val_type: Some(val_type),
                    depth: 0,
                };
                return;
            }
        }
        // Doesn't exist, define a new one
        globals.push(VarData {
            name,
            val_type: Some(val_type),
            depth: 0,
        });
    }

    fn declare_local(&mut self, name: String, val_type: Option<ValueType>) {
        self.local_bindings.push(VarData {
            name,
            val_type,
            depth: self.depth,
        });
    }

    fn enter_scope(&mut self) {
        self.depth += 1;
    }

    fn exit_scope(&mut self) {
        self.depth -= 1;
        while let Some(top) = self.local_bindings.last() {
            if top.depth <= self.depth {
                break;
            } else {
                self.local_bindings.pop();
            }
        }
    }

    fn in_global_scope(&self) -> bool {
        self.is_global && self.depth == 0
    }

    fn error_at_token(&mut self, token: &Token, reason: UsageError) {
        self.globals.borrow_mut().had_error = true;
        eprintln!("[unfinished] Error on line {}: {:?}", token.line(), reason);
    }

    fn error_at_expr(&mut self, expr: &Expr, reason: UsageError) {
        self.globals.borrow_mut().had_error = true;
        eprintln!("[unfinished] Error: {:?}", reason);
        // TODO actual error locating
    }

    fn error_msg_at_expr(&mut self, expr: &Expr, reason: UsageError, msg: &str) {
        self.error_at_expr(expr, reason);
        eprintln!("\t{}", msg);
    }

    /// Returns the type of the given expression.
    fn expect_type(&mut self, expected: &ValueType, actual: &Expr) -> ValueType {
        // TODO stuff with any and casting during runtime i think
        let actual_type = self.resolve_expr(&actual);
        if *expected != actual_type {
            self.error_msg_at_expr(
                &actual,
                UsageError::TypeError,
                &format!("Expected {:?} but got {:?}", expected, actual_type),
            );
        }
        actual_type
    }

    fn finish_function(&mut self, name: &String) {
        let Some(func) = self.function_backlog.remove(name) else {
            // Assume the function has already been resolved at a previous call
            return;
        };
        let TempFunction {
            ret_type,
            params,
            body,
        } = func;
        let mut inner = FunctionResolver::new(false);
        inner.globals = self.globals.clone();
        inner.ret_type = ret_type.clone();
        for (name, pm_type) in params {
            inner.local_bindings.push(VarData {
                name: name.copy_ident(),
                val_type: Some(pm_type.clone()),
                depth: 0,
            });
        }
        for stmt in body {
            inner.resolve_stmt(stmt);
        }
    }
}

impl<'ast> StmtVisitor<'ast, ()> for FunctionResolver<'ast> {
    fn visit_summon_stmt(&mut self, path: &Vec<Token>, alias: &Option<Token>, id: usize) {
        todo!() // TODO
    }

    fn visit_var_stmt(&mut self, name: &Token, val: &'ast Option<Box<Expr>>) {
        let val_type = val.as_ref().map(|v| self.resolve_expr(v));
        if self.in_global_scope() {
            let Some(real_type) = val_type else {
                self.error_at_token(name, UsageError::MustInitGlobal);
                return;
            };
            self.declare_global(name.copy_ident(), real_type);
        } else {
            self.declare_local(name.copy_ident(), val_type);
        }
    }

    fn visit_block_stmt(&mut self, statements: &'ast Vec<Stmt>) {
        self.enter_scope();
        for stmt in statements {
            self.resolve_stmt(stmt);
        }
        self.exit_scope();
    }

    fn visit_expression_stmt(&mut self, expression: &'ast Box<Expr>) {
        self.resolve_expr(expression);
    }

    fn visit_if_stmt(
        &mut self,
        condition: &'ast Box<Expr>,
        true_branch: &'ast Box<Stmt>,
        false_branch: &'ast Option<Box<Stmt>>,
    ) {
        self.expect_type(&ValueType::Bool, condition);
        todo!() // TODO
        // self.resolve_stmt(true_branch);
        // if let Some(false_branch) = false_branch {
        //     self.resolve_stmt(false_branch);
        // }
    }

    fn visit_while_stmt(&mut self, condition: &Box<Expr>, body: &'ast Box<Stmt>) {
        self.expect_type(&ValueType::Bool, condition);
        self.loop_depth += 1;
        self.resolve_stmt(body);
        self.loop_depth -= 1;
    }

    fn visit_for_stmt(&mut self, var: &Token, sequence: &Box<Expr>, body: &Box<Stmt>) {
        // TODO an actual range or sequence type
        // self.loop_depth += 1;
        todo!() // TODO;
        // self.loop_depth -= 1;
    }

    fn visit_keyword_stmt(&mut self, keyword: &Token, arg: &Option<Box<Expr>>) {
        match keyword.kind() {
            TokenType::Break | TokenType::Continue => {
                let mut loops_to_jump = 1;
                if let Some(arg) = arg {
                    let Expr::Literal { val, .. } = &**arg else {
                        self.error_at_expr(arg, UsageError::InvalidLoopControl);
                        return;
                    };
                    let Value::Int(i) = *val else {
                        self.error_at_expr(arg, UsageError::InvalidLoopControl);
                        return;
                    };
                    if i < 1 {
                        self.error_at_expr(arg, UsageError::InvalidLoopControl);
                        return;
                    }
                    loops_to_jump = i as usize;
                }
                if loops_to_jump > self.loop_depth {
                    self.error_at_token(keyword, UsageError::InvalidLoopControl);
                }
            }
            TokenType::Return => {
                // TODO actually do this please
                if let Some(arg) = arg {
                    self.resolve_expr(arg);
                }
            }
            _ => panic!("Invalid token for keyword statement"),
        }
    }

    fn visit_function_stmt(
        &mut self,
        ret_type: &'ast ValueType,
        name: &'ast Token,
        params: &'ast Vec<(Token, ValueType)>,
        body: &'ast Vec<Stmt>,
        _id: usize,
    ) {
        if self.in_global_scope() {
            self.declare_global(name.copy_ident(), ValueType::func_type(ret_type, params));
        } else {
            self.declare_local(
                name.copy_ident(),
                Some(ValueType::func_type(ret_type, params)),
            );
        }
        // Wait for resolving until first call
        // No forward declarations in this household
        self.function_backlog.insert(
            name.copy_ident(),
            TempFunction {
                ret_type,
                params,
                body,
            },
        );
    }
}

impl<'ast> ExprVisitor<'_, ValueType> for FunctionResolver<'ast> {
    /// Condition must be bool, two outcomes must be the same value
    fn visit_conditional_expr(
        &mut self,
        left: &Box<Expr>,
        middle: &Box<Expr>,
        right: &Box<Expr>,
        _id: usize,
    ) -> ValueType {
        self.expect_type(&ValueType::Bool, left);
        let true_type = self.resolve_expr(&middle);
        let false_type = self.expect_type(&true_type, &right);
        if true_type != false_type {
            ValueType::Any
        } else {
            true_type
        }
    }

    fn visit_binary_expr(
        &mut self,
        left: &Box<Expr>,
        op: &Token,
        right: &Box<Expr>,
        id: usize,
    ) -> ValueType {
        let left = self.resolve_expr(left);
        let right = self.resolve_expr(right);
        let Some((both, _)) = ValueType::coerce(&left, &right) else {
            self.error_at_token(op, UsageError::IncompatibleTypes);
            return ValueType::Any;
        };
        self.globals
            .borrow_mut()
            .final_data
            .bin_types
            .insert(id, both.clone());
        if [TokenType::DoubleEqual, TokenType::BangEqual].contains(op.kind()) {
            // supported for all types probably
            return ValueType::Bool;
        }
        // after this point, both are the same type
        match both {
            ValueType::Any => ValueType::Any,
            ValueType::Int => match op.kind() {
                TokenType::Plus
                | TokenType::Minus
                | TokenType::Star
                | TokenType::FSlash
                | TokenType::Percent
                | TokenType::DoubleLess
                | TokenType::DoubleGreater
                | TokenType::Ampersand
                | TokenType::Carrot
                | TokenType::VertBar => ValueType::Int,
                TokenType::Less
                | TokenType::Greater
                | TokenType::GreaterEqual
                | TokenType::LessEqual => ValueType::Bool,
                TokenType::DoubleDot => todo!(), // TODO
                _ => {
                    self.error_at_token(op, UsageError::InvalidOperator);
                    ValueType::Int
                }
            },
            ValueType::Float => match op.kind() {
                TokenType::Plus
                | TokenType::Minus
                | TokenType::Star
                | TokenType::FSlash
                | TokenType::Percent => ValueType::Float,
                TokenType::Less
                | TokenType::Greater
                | TokenType::GreaterEqual
                | TokenType::LessEqual => ValueType::Bool,
                TokenType::DoubleDot => todo!(), // TODO
                _ => {
                    self.error_at_token(op, UsageError::InvalidOperator);
                    ValueType::Float
                }
            },
            ValueType::Char => {
                self.error_at_token(op, UsageError::InvalidOperator);
                // assume binary collapses to one value of same type
                ValueType::Char
            }
            ValueType::Bool => match op.kind() {
                TokenType::DoubleAmpersand | TokenType::DoubleVertBar => ValueType::Bool,
                _ => {
                    self.error_at_token(op, UsageError::InvalidOperator);
                    ValueType::Bool
                }
            },
            ValueType::Object(obj_type) => match obj_type {
                ObjectType::String => match op.kind() {
                    TokenType::Plus => ValueType::Object(ObjectType::String),
                    TokenType::Less
                    | TokenType::Greater
                    | TokenType::DoubleLess
                    | TokenType::DoubleGreater
                    | TokenType::GreaterEqual
                    | TokenType::LessEqual => ValueType::Bool,
                    TokenType::DoubleDot => todo!(), // TODO
                    _ => {
                        self.error_at_token(op, UsageError::InvalidOperator);
                        ValueType::Object(ObjectType::String)
                    }
                },
                ObjectType::Function { .. } => panic!("can't coerce to function??")
            },
            ValueType::None => {
                self.error_at_token(op, UsageError::InvalidOperator);
                // assume binary collapses to one value of same type
                ValueType::None
            }
        }
    }

    fn visit_assign_expr(
        &mut self,
        assignee: &Box<Expr>,
        value: &Box<Expr>,
        id: usize,
    ) -> ValueType {
        let val_type = self.resolve_expr(&value);
        // rust i swear to god
        match &**assignee {
            // TODO other valid assignees (get, index)
            Expr::Variable { identifier, .. } => {
                match self.get_var(identifier.lexeme().unwrap()) {
                    Some((binding, data)) => {
                        self.globals.borrow_mut().final_data.bindings.insert(id, binding);
                        if let Some(var_type) = data.val_type && var_type != val_type {
                            self.error_at_expr(value, UsageError::TypeError);
                        }
                    }
                    None => self.error_at_expr(assignee, UsageError::UndefinedIdent),
                }
            }
            _ => self.error_at_expr(&assignee, UsageError::InvalidAssign),
        }
        val_type
    }

    fn visit_cast_expr(&mut self, expr: &Box<Expr>, new_type: &ValueType, id: usize) -> ValueType {
        let old_type = self.resolve_expr(expr);
        todo!(); // TODO
        new_type.clone()
    }

    fn visit_unary_expr(
        &mut self,
        op: &Token,
        target: &Box<Expr>,
        prefix: &bool,
        id: usize,
    ) -> ValueType {
        let targ_type = self.resolve_expr(target);
        match targ_type {
            ValueType::Int => {
                // negate & bitwise not
                return ValueType::Int
            },
            ValueType::Float => {
                // negate
                return ValueType::Float
            },
            ValueType::Bool => {
                // logic not
                return ValueType::Bool
            },
            _ => self.error_at_expr(target, UsageError::TypeError),
        }
        targ_type
    }

    fn visit_call_expr(&mut self, callee: &Box<Expr>, args: &Vec<Expr>, id: usize) -> ValueType {
        let callee_type = self.resolve_expr(&callee);
        if let ValueType::Object(obj) = &callee_type
            && let ObjectType::Function { ret_type, params } = obj
        {
            let mut args_match = true;
            for i in 0..args.len() {
                if self.resolve_expr(&args[i]) != params[i] {
                    args_match = false;
                    self.error_at_expr(&args[i], UsageError::ParamMismatch);
                }
            }
            if args_match {
                // everything is correct, resolve body
                // find identifier first
                let ident = match &**callee {
                    // TODO other expressions that can eval to a func
                    Expr::Variable { identifier, .. } => identifier.copy_ident(),
                    _ => String::new(),
                };
                self.finish_function(&ident);
            }
            *ret_type.clone()
        } else {
            self.error_msg_at_expr(
                &callee,
                UsageError::TypeError,
                &format!("Can't call type {}", callee_type),
            );
            ValueType::Any
        }
    }

    fn visit_variable_expr(&mut self, identifier: &Token, id: usize) -> ValueType {
        let var_type = self.get_var(identifier.lexeme().unwrap());
        match var_type {
            Some((binding, data)) => {
                self.globals
                    .borrow_mut()
                    .final_data
                    .bindings
                    .insert(id, binding);
                let Some(initialized_type) = data.val_type else {
                    self.error_at_token(identifier, UsageError::UndefinedIdent);
                    return ValueType::Any;
                };
                initialized_type
            }
            None => {
                self.error_at_token(identifier, UsageError::UndefinedIdent);
                ValueType::Any
            }
        }
    }

    fn visit_literal_expr(&mut self, repr: &Token, val: &Value, _id: usize) -> ValueType {
        self.value_count += 1;
        if self.value_count == u8::MAX as usize {
            // allow
            self.error_at_token(repr, UsageError::TooManyConstants);
        }
        val.get_type()
    }
}
