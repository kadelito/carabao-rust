use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::builtins::GLOBAL_FUNCS;
use crate::expr_ast::*;
use crate::lexing::{Token, TokenType};
use crate::parsing::BINARY_OPERATORS;
use crate::stmt_ast::{Stmt, StmtVisitor};
use crate::values::*;
use crate::types::*;

pub fn analyze(program: &Vec<Stmt>) -> Result<AnalysisResult, Vec<UsageError>> {
    // Initialize stuff
    let globals = Rc::new(RefCell::new(Globals::new()));
    let mut resolver = FunctionResolver::new(true);
    resolver.globals = globals;

    for (name, obj) in GLOBAL_FUNCS {
        resolver.declare_global((*name).to_owned(), Value::from(obj).get_type());
    }

    resolver.resolve(program)
}

/// All hashmaps with `usize` keys correspond to AST node `id`s.
pub struct AnalysisResult {
    pub bindings: HashMap<usize, Binding>,
    /// Maps `Expr::Binary` `id`s to the type both operands should be.
    /// 
    /// This field exists because binary coercions have different rules
    /// from singularly differing `ValueType`s
    /// (mainly bc string concatention)
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

#[derive(Debug, Clone, Copy, PartialEq)]
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
    TooManyArgs,
    CantIndexThat,
}

struct FunctionResolver<'ast> {
    value_count: usize,
    line: u32,
    ret_type: ValueType,
    local_bindings: Vec<VarData>,
    depth: usize,
    is_global: bool,
    loop_depth: usize,
    function_backlog: HashMap<String, TempFunction<'ast>>,

    globals: Rc<RefCell<Globals>>,
}

#[derive(Debug)]
struct TempFunction<'ast> {
    name: &'ast str,
    ret_type: &'ast ValueType,
    params: &'ast Vec<(Token, ValueType)>,
    body: &'ast Vec<Stmt>,
}

struct Globals {
    errors: Vec<UsageError>,
    /// Global variables only.
    /// No main script locals or variables in functions.
    bindings: Vec<VarData>,
    final_data: AnalysisResult,
}

impl Globals {
    fn new() -> Self {
        Self {
            bindings: Vec::new(),
            errors: Vec::new(),
            final_data: AnalysisResult::new(),
        }
    }
}

#[derive(Clone)]
struct VarData {
    pub name: String,
    pub val_type: ValueType,
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
            line: 0,
        }
    }

    pub fn resolve(&mut self, stmts: &'ast Vec<Stmt>) -> Result<AnalysisResult, Vec<UsageError>> {
        for stmt in stmts {
            self.resolve_stmt(stmt);
        }
        // Resolve functions that are never called
        // We assume the global state they use is at the end of execution
        // (aka after all global declarations & redeclarations)
        for unfinished in std::mem::replace(&mut self.function_backlog, HashMap::new()) {
            let (_, func) = unfinished;
            self.finish_function(func);
        }
        if !self.globals.borrow().errors.is_empty() {
            Err(std::mem::replace(&mut self.globals.borrow_mut().errors, Vec::new()))
        } else {
            let data = std::mem::replace(
                &mut self.globals.borrow_mut().final_data,
                AnalysisResult::new(),
            );
            Ok(data)
        }
    }

    fn resolve_stmt(&mut self, stmt: &'ast Stmt) {
        stmt.accept(self)
    }
    
    fn resolve_expr(&mut self, expr: &Expr) -> ValueType {
        let expr_type = expr.accept(self);
        // Save type of each sub-expression
        self.globals.borrow_mut().final_data.expr_types.insert(expr.id(), expr_type.clone());
        expr_type
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
        for (i, data) in self.globals.borrow().bindings.iter().enumerate().rev() {
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
        globals.push(VarData {
            name,
            val_type: val_type,
            depth: 0,
        });
    }

    fn declare_local(&mut self, name: String, val_type: ValueType) {
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
        eprintln!("[unfinished] Error on line {}: {:?}", token.line(), reason);
    }
    
    fn error_at_expr(&mut self, _expr: &Expr, reason: UsageError) {
        eprintln!("[unfinished] Error on line {}: {:?}", self.line, reason);
        self.globals.borrow_mut().errors.push(reason);
        // TODO actual error locating
    }

    fn error_msg_at_expr(&mut self, expr: &Expr, reason: UsageError, msg: &str) {
        self.error_at_expr(expr, reason);
        eprintln!("\t{}", msg);
    }

    /// Evaluates and returns the actual type of the given expression,
    /// displaying an error if there is a mismatch.
    /// 
    /// See `expect_resolved_type` for more details
    fn expect_type(&mut self, expected: &ValueType, actual: &Expr) -> bool {
        let actual_type = self.resolve_expr(actual);
        self.expect_resolved_type(expected, actual, &actual_type)
    }

    /// Returns the actual type of the already-evaluated expression,
    /// displaying an error if there is a mismatch.
    /// 
    /// This function uses `ValueType::can_convert_type`,
    /// which means it uses coercion rules to determine whether the assertion passes.
    /// For example, an `int` is allowed where a `float` is expected.
    /// Actual casting is done during `codegen`.
    fn expect_resolved_type(&mut self, expected: &ValueType, actual: &Expr, actual_type: &ValueType) -> bool {
        let valid = ValueType::can_convert_type(expected, actual_type);
        if !valid {
            self.error_msg_at_expr(
                &actual,
                UsageError::TypeError,
                &format!("Expected {:?} but got {:?}", expected, actual_type),
            );
        }
        valid
    }

    fn finish_function_by_name(&mut self, name: &String) {
        if let Some(func) = self.function_backlog.remove(name) {
            self.finish_function(func);
        };
        // Assume the function has already been resolved at a previous call
    }

    fn finish_function(&mut self, func: TempFunction) {
        let TempFunction {
            name,
            ret_type,
            params,
            body,
        } = func;
        let mut inner = FunctionResolver::new(false);
        inner.globals = self.globals.clone();
        inner.ret_type = ret_type.clone();

        inner.local_bindings.push(VarData {
            name: name.to_owned(),
            val_type: ValueType::func_type(ret_type, params),
            depth: 0
        });
        for (name, pm_type) in params {
            inner.local_bindings.push(VarData {
                name: name.copy_ident(),
                val_type: pm_type.clone(),
                depth: 0,
            });
        }
        for stmt in body {
            inner.resolve_stmt(stmt);
        }
    }
}

impl<'ast> StmtVisitor<'ast, ()> for FunctionResolver<'ast> {
    fn visit_summon_stmt(&mut self, _path: &Vec<Token>, _alias: &Option<Token>, _id: usize) {
        todo!() // TODO summons
    }

    fn visit_var_stmt(&mut self, name: &Token, explicit_type: &Option<ValueType>, value: &'ast Option<Box<Expr>>) {

        self.line = name.line();

        let real_type = match (explicit_type, value) {
            // give it the any type
            (None, None) => ValueType::Any,
            // it has the type of `val`
            (None, Some(val)) => self.resolve_expr(val),
            // it has `explicit_type`, will be given a dummy value
            (Some(var), None) => var.clone(),
            // it has `explicit_type`, will be given `val` casted to `explicit_type`
            (Some(var), Some(val)) => {
                self.expect_type(var, val);
                var.clone()
            },
        };

        if self.in_global_scope() {
            self.declare_global(name.copy_ident(), real_type);
        } else {
            self.declare_local(name.copy_ident(), real_type);
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
        // TODO control flow analysis
        self.resolve_stmt(true_branch);
        if let Some(false_branch) = false_branch {
            self.resolve_stmt(false_branch);
        }
    }

    fn visit_while_stmt(&mut self, condition: &Box<Expr>, body: &'ast Box<Stmt>) {
        self.expect_type(&ValueType::Bool, condition);
        self.loop_depth += 1;
        self.resolve_stmt(body);
        self.loop_depth -= 1;
    }

    fn visit_for_stmt(&mut self, var: &Token, _sequence: &Box<Expr>, _body: &Box<Stmt>) {

        self.line = var.line();

        // TODO an actual range or sequence type
        // self.loop_depth += 1;
        todo!("idk :3")
        // self.loop_depth -= 1;
    }

    fn visit_keyword_stmt(&mut self, keyword: &Token, arg: &Option<Box<Expr>>) {
        self.line = keyword.line();

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
                let ret_type = self.ret_type.clone();
                if let Some(arg) = arg {
                    self.expect_type(&ret_type, arg);   
                } else {
                    // cheat a little bit and fake an expression
                    self.expect_type(&ret_type,
                        &Expr::Literal { repr: keyword.clone(), val: Value::None, id: 0 });   
                }
            }
            _ => panic!("Invalid token for keyword statement made it to analysis"),
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
        self.line = name.line();

        let temp_name = name.copy_ident();
        let val_type = ValueType::func_type(ret_type, params);
        if self.in_global_scope() {
            self.declare_global(temp_name, val_type);
        } else {
            self.declare_local(temp_name, val_type)
        }
        // Wait for resolving until first call
        // No forward declarations in this household
        self.function_backlog.insert(
            name.copy_ident(),
            TempFunction {
                name: name.lexeme().unwrap(),
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
        let true_type = self.resolve_expr(middle);
        let false_type = self.resolve_expr(right);
        ValueType::coerce_binary(&true_type, TokenType::DoubleEqual, &false_type)
            .unwrap_or({
                // not equal & couldn't coerce
                self.expect_resolved_type(&true_type, right, &false_type);
                (ValueType::Any, ValueType::Any)
            }).0 // types are equal after this point
    }

    fn visit_binary_expr(
        &mut self,
        left: &Box<Expr>,
        op: &Token,
        right: &Box<Expr>,
        id: usize,
    ) -> ValueType {
        self.line = op.line();

        let left = self.resolve_expr(left);
        let right = self.resolve_expr(right);
        let Some((both, _)) = ValueType::coerce_binary(&left, op.kind(), &right) else {
            self.error_at_token(op, UsageError::IncompatibleTypes);
            return ValueType::Any;
        };
        self.globals
            .borrow_mut()
            .final_data
            .bin_types
            .insert(id, both.clone());
        if [TokenType::DoubleEqual, TokenType::BangEqual].contains(&op.kind()) {
            // supported for all types probably
            return ValueType::Bool;
        }
        // after this point, both are the same type
        match both {
            ValueType::Any => todo!("any in binary operators"),
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
                TokenType::DoubleDot => todo!(), // TODO ranges
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
                TokenType::DoubleDot => todo!(), // TODO ranges
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
            ValueType::String => match op.kind() {
                TokenType::Plus => ValueType::String,
                TokenType::Less
                | TokenType::Greater
                | TokenType::DoubleLess
                | TokenType::DoubleGreater
                | TokenType::GreaterEqual
                | TokenType::LessEqual => ValueType::Bool,
                TokenType::DoubleDot => todo!(), // TODO ranges
                _ => {
                    self.error_at_token(op, UsageError::InvalidOperator);
                    ValueType::String
                }
            },
            ValueType::Function { .. } => panic!("can't coerce to function??"),
            ValueType::None => {
                self.error_at_token(op, UsageError::InvalidOperator);
                // assume binary collapses to one value of same type
                ValueType::None
            }
            ValueType::List(value_type) => todo!("List concatenation"),
        }
    }

    fn visit_assign_expr(
        &mut self,
        assignee: &Box<Expr>,
        value: &Box<Expr>,
        assign_id: usize,
    ) -> ValueType {
        let val_type = self.resolve_expr(&value);
        // rust i swear to god
        match &**assignee {
            Expr::Variable { identifier, id: var_id } => {
                match self.get_var(identifier.lexeme().unwrap()) {
                    Some((binding, data)) => {
                        // record id of outermost assign, not the assignee
                        self.globals.borrow_mut().final_data.bindings.insert(assign_id, binding);
                        self.expect_resolved_type(&data.val_type, value, &val_type);
                        self.globals.borrow_mut().final_data.expr_types.insert(*var_id, data.val_type.clone());
                        return data.val_type;
                    }
                    None => self.error_at_expr(assignee, UsageError::UndefinedIdent),
                }
            }
            Expr::Get { obj, property, .. } => {
                todo!()
            }
            Expr::Slice { sequence, query, .. } => {
                todo!()
            }
            _ => self.error_at_expr(&assignee, UsageError::InvalidAssign),
        }
        val_type
    }

    fn visit_cast_expr(&mut self, expr: &Box<Expr>, new_type: &ValueType, _id: usize) -> ValueType {
        let old_type = self.resolve_expr(expr);
        if !ValueType::can_convert_type(new_type, &old_type) {
            self.error_at_expr(expr, UsageError::TypeError);
        }
        new_type.clone()
    }

    fn visit_unary_expr(
        &mut self,
        op: &Token, // theres only 3 operators and they're all very similar
        target: &Box<Expr>,
        _prefix: &bool,
        _id: usize,
    ) -> ValueType {
        self.line = op.line();

        let target_type = self.resolve_expr(target);
        #[cfg(test)]
        match op.kind() {
            TokenType::DoubleGreater  => { return target_type; }
            TokenType::DoubleLess => { return ValueType::None; }
            _ => {}
        }
        match target_type {
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
        target_type
    }

    fn visit_call_expr(&mut self, callee: &Box<Expr>, args: &Vec<Expr>, _id: usize) -> ValueType {
        // TODO Other valid callees? auto constructors
        if args.len() >= u8::MAX.into() {
            self.error_at_expr(args.last().unwrap(), UsageError::TooManyArgs);
        }
        match self.resolve_expr(&callee) {
            ValueType::Function { ret_type, params } => {
                let mut args_match = true;
                if args.len() != params.len() {
                    self.error_at_expr(callee, UsageError::ParamMismatch);
                } else {
                    for (param, arg) in params.iter().zip(args.iter()) {
                        let arg_type = self.resolve_expr(arg);
                        self.expect_resolved_type(param, arg, &arg_type);
                        if !ValueType::can_convert_type(&arg_type, param) {
                            args_match = false;
                            self.error_at_expr(arg, UsageError::ParamMismatch);
                        }
                    }
                }
                if args_match {
                    // everything is correct, resolve body
                    // find identifier first
                    let ident = match &**callee {
                        // TODO other expressions that can eval to a func
                        // anonymous?
                        Expr::Variable { identifier, .. } => identifier.copy_ident(),
                        _ => String::new(),
                    };
                    self.finish_function_by_name(&ident);
                }
                *ret_type.clone()
            }
            other => {
                self.error_msg_at_expr(
                    &callee,
                    UsageError::TypeError,
                    &format!("Can't call type {}", other),
                );
                ValueType::Any
            }
        }
    }

    fn visit_variable_expr(&mut self, identifier: &Token, id: usize) -> ValueType {
        self.line = identifier.line();

        let var_type = self.get_var(identifier.lexeme().unwrap());
        match var_type {
            Some((binding, data)) => {
                self.globals
                    .borrow_mut()
                    .final_data
                    .bindings
                    .insert(id, binding);
                data.val_type
            }
            None => {
                self.error_at_token(identifier, UsageError::UndefinedIdent);
                ValueType::Any
            }
        }
    }

    fn visit_literal_expr(&mut self, repr: &Token, val: &Value, _id: usize) -> ValueType {
        self.line = repr.line();

        self.value_count += 1;
        if self.value_count == u8::MAX as usize {
            // TODO increase max constants (maybe to u32? any more and its like. whats goin on) and have a OP_CONSTANT_WIDE
            self.error_at_token(repr, UsageError::TooManyConstants);
        }
        val.get_type()
    }
    
    fn visit_boolean_expr(&mut self,
        left: &'_ Box<Expr>, op: &'_ Token, right: &'_ Box<Expr>, _id: usize) -> ValueType {
        self.line = op.line();

        self.expect_type(&ValueType::Bool, left);
        self.expect_type(&ValueType::Bool, right);
        ValueType::Bool
    }
    
    fn visit_slice_expr(&mut self,
        sequence: &'_ Box<Expr>, query: &'_ Box<Expr>, id: usize) -> ValueType {
        let seq_type = self.resolve_expr(sequence);
        return match seq_type {
            // TODO Overload slicing for objects?
            // maybe (a: int)[b: int] to apply bitmask?
            // (a & (1 << b) == 1)
            ValueType::String => todo!(),
            ValueType::List(value_type) => todo!(),
            _ => {
                self.error_at_expr(sequence, UsageError::CantIndexThat);
                ValueType::Any
            },
        }
    }
    
    fn visit_method_expr(&mut self,
        obj: &'_ Box<Expr>, method: &'_ Token, args: &'_ Vec<Expr>, id: usize) -> ValueType {
        self.line = method.line();

        todo!()
    }
    
    fn visit_get_expr(&mut self,
        obj: &'_ Box<Expr>, property: &'_ Token, id: usize) -> ValueType {
        self.line = property.line();

        todo!()
    }
    
    fn visit_list_expr(&mut self, items: &'_ Vec<Expr>, _id: usize) -> ValueType {
        if items.is_empty() {
            return ValueType::List(Box::new(ValueType::Any));
        }
        let first_type = self.resolve_expr(&items[0]);
        // TODO weird type expectations
        // [1, 2.0] fails but [1.0, 2] is fine?
        for item in &items[1..] {
            if !self.expect_type(&first_type, item) {
                return ValueType::List(Box::new(ValueType::Any));
            }
        }
        ValueType::List(Box::new(first_type))
    }
}
