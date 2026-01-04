use std::collections::HashMap;

use crate::errors::macros::internal_error;
use crate::expr_ast::*;
use crate::lexing::{Token, TokenType};
use crate::registry::GLOBAL_FUNCS;
use crate::stmt_ast::{Stmt, StmtVisitor};
use crate::typed_values::*;
use crate::types::*;

pub fn analyze(program: &Vec<Stmt>) -> Result<AnalysisResult, Vec<UsageError>> {
    let mut resolver = Resolver::new();

    for (name, obj) in GLOBAL_FUNCS.iter() {
        resolver.declare_global(
            (*name).to_owned(),
            if name.is_empty() {
                // these will never appear in user code, just here to preserve spacing
                ValueType::Unchecked
            } else {
                obj.get_type()
            },
        );
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
    MustInitGlobal,
    InvalidLoopControl,
    IncompatibleTypes,
    InvalidOperator,
    TooManyConstants,
    TooManyArgs,
    CantIndexThat,
    NotIterable,
    InvalidRange,
    ParamMismatch,
    WhichFunction,
    DontGotFields,
    NoSuchField,
    CantCallThat,
    InvalidSlice,
}

struct FunctionContext<'ast> {
    value_count: usize,
    line: u32,
    ret_type: ValueType, // TODO replace with an Option for inferrence (and in TempFunction)
    local_bindings: Vec<VarData>,
    depth: usize,
    is_global: bool,
    loop_depth: usize,
    function_backlog: HashMap<String, Vec<Option<TempFunction<'ast>>>>,
}

impl FunctionContext<'_> {
    pub fn new(is_global: bool, ret_type: ValueType) -> Self {
        Self {
            ret_type,
            value_count: 0,
            local_bindings: Vec::new(),
            depth: 0,
            is_global,
            loop_depth: 0,
            function_backlog: HashMap::new(),
            line: 0,
        }
    }
}

#[derive(Debug)]
struct TempFunction<'ast> {
    name: &'ast str,
    ret_type: &'ast ValueType, // see previous TODO
    params: &'ast Vec<(Token, ValueType)>,
    body: &'ast Vec<Stmt>,
}

struct Resolver<'ast> {
    context: FunctionContext<'ast>,

    errors: Vec<UsageError>,
    /// Global variables only.
    /// No main script locals or variables in functions.
    global_bindings: Vec<VarData>,
    final_data: AnalysisResult,
}

impl Resolver<'_> {
    fn new() -> Self {
        Self {
            context: FunctionContext::new(true, ValueType::None),
            global_bindings: Vec::new(),
            errors: Vec::new(),
            final_data: AnalysisResult::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct VarData {
    name: String,
    val_type: ValueType,
    depth: usize,
}

impl<'ast> Resolver<'ast> {
    pub fn resolve(&mut self, stmts: &'ast Vec<Stmt>) -> Result<AnalysisResult, Vec<UsageError>> {
        for stmt in stmts {
            self.resolve_stmt(stmt);
        }
        // Resolve functions that are never called
        // We assume the global state they use is at the end of execution
        // (aka after all global declarations & redeclarations)
        for unfinished in std::mem::replace(&mut self.context.function_backlog, HashMap::new()) {
            let (_, funcs) = unfinished;
            for overload in funcs {
                if let Some(func) = overload {
                    self.finish_function(func);
                }
            }
        }
        if !self.errors.is_empty() {
            Err(std::mem::replace(&mut self.errors, Vec::new()))
        } else {
            let data = std::mem::replace(&mut self.final_data, AnalysisResult::new());
            Ok(data)
        }
    }

    fn resolve_stmt(&mut self, stmt: &'ast Stmt) {
        stmt.accept(self)
    }

    fn resolve_expr(&mut self, expr: &Expr) -> ValueType {
        let expr_type = expr.accept(self);
        // Save type of each sub-expression
        self.final_data
            .expr_types
            .insert(expr.id(), expr_type.clone());
        expr_type
    }

    fn get_identifier(&self, ident: &str) -> Option<(Binding, &VarData)> {
        use Binding::*;

        // Look in locals first
        for (slot, data) in self.context.local_bindings.iter().enumerate().rev() {
            if data.name == *ident {
                return Some((Stack(slot), data));
            }
        }

        // TODO class fields?

        // Look in globals next
        for (slot, data) in self.global_bindings.iter().enumerate().rev() {
            if data.name == *ident {
                return Some((Globals(slot), data));
            }
        }

        None
    }

    // Returns all accessible info corresponding to an identifier,
    // in order of declaration.
    fn get_all_identifiers(&self, ident: &str) -> Vec<(Binding, &VarData)> {
        use Binding::*;

        let mut declarations = Vec::new();

        // Look in locals first
        for (slot, data) in self.context.local_bindings.iter().enumerate() {
            if data.name == *ident {
                declarations.push((Stack(slot), data));
            }
        }

        // Look in globals next
        for (slot, data) in self.global_bindings.iter().enumerate() {
            if data.name == *ident {
                declarations.push((Globals(slot), data));
            }
        }

        declarations
    }

    fn declare_global(&mut self, name: String, val_type: ValueType) {
        self.global_bindings.push(VarData {
            name,
            val_type,
            depth: 0,
        });
    }

    fn declare_local(&mut self, name: String, val_type: ValueType) {
        self.context.local_bindings.push(VarData {
            name,
            val_type,
            depth: self.context.depth,
        });
    }

    fn enter_scope(&mut self) {
        self.context.depth += 1;
    }

    fn exit_scope(&mut self) {
        self.context.depth -= 1;
        while let Some(top) = self.context.local_bindings.last()
            && top.depth > self.context.depth
        {
            self.context.local_bindings.pop();
        }
    }

    fn in_global_scope(&self) -> bool {
        self.context.is_global && self.context.depth == 0
    }

    fn error_at_token(&mut self, token: &Token, reason: UsageError) -> ValueType {
        eprintln!("[unfinished] Error on line {}: {:?}", token.line(), reason);
        self.errors.push(reason);
        ValueType::Unchecked
    }

    fn error_at_expr(&mut self, _expr: &Expr, reason: UsageError) -> ValueType {
        // TODO actual error locating
        eprintln!(
            "[unfinished] Error on line {}: {:?}",
            self.context.line, reason
        );
        self.errors.push(reason);
        ValueType::Unchecked
    }

    fn error_msg_at_expr(&mut self, expr: &Expr, reason: UsageError, msg: &str) -> ValueType {
        self.error_at_expr(expr, reason);
        eprintln!("\t{}", msg);
        ValueType::Unchecked
    }

    /// Evaluates and returns whether the expression can convert to `expected`,
    /// displaying an error if there is a mismatch.
    ///
    /// See `expect_resolved_type` for more details
    fn expect_type(&mut self, expected: &ValueType, actual: &Expr) -> bool {
        let actual_type = self.resolve_expr(actual);
        self.expect_resolved_type(expected, actual, &actual_type)
    }

    /// Returns whether the already-evaluated `actual` can slot in
    /// for the type `expected`, displaying an error if not.
    ///
    /// This function uses `ValueType::can_convert_type`,
    /// which means it uses coercion rules to determine whether the assertion passes.
    /// For example, an `int` is allowed where a `float` is expected.
    /// Actual casting is done during `codegen`.
    fn expect_resolved_type(
        &mut self,
        expected: &ValueType,
        actual: &Expr,
        actual_type: &ValueType,
    ) -> bool {
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

    fn finish_function(&mut self, func: TempFunction<'ast>) {
        let TempFunction {
            name,
            ret_type,
            params,
            body,
        } = func;
        let inner = FunctionContext::new(false, ret_type.clone());

        let old = std::mem::replace(&mut self.context, inner);

        self.declare_local(name.to_owned(), ValueType::func_type(ret_type, params));
        for (name, pm_type) in params {
            self.declare_local(name.copy_ident(), pm_type.clone());
        }
        for stmt in body {
            self.resolve_stmt(stmt);
        }
        // TODO ensure a valid type is returned in every control flow path

        self.context = old;
    }

    // Resolves an identifier being called.
    fn resolve_ident_call(
        &mut self,
        identifier: &Token,
        callee_id: usize,
        args: &[&Expr],
    ) -> ValueType {
        // do this first bc `definitions` borrows from self as well
        let arg_types = args
            .into_iter()
            .map(|arg| self.resolve_expr(arg))
            .collect::<Box<[ValueType]>>();

        let mut definitions = self.get_all_identifiers(identifier.lexeme().unwrap());
        if definitions.is_empty() {
            return self.error_at_token(identifier, UsageError::UndefinedIdent);
        }

        definitions.retain(|(_, data)| matches!(data.val_type, ValueType::Function(_)));
        if definitions.is_empty() {
            // No functions after filtering
            return self.error_at_token(identifier, UsageError::CantCallThat);
        }

        // Try to match the correct function (chronologically descending)
        let mut correct_index = 0;
        let mut correct_overload = None;

        // Check for exact type matches
        for (binding, data) in definitions.iter().rev() {
            let ValueType::Function(func) = &data.val_type else {
                internal_error!("");
            };

            if func.params == arg_types {
                correct_overload = Some((binding.clone(), func.clone()));
                break;
            }
            correct_index += 1;
        }

        if correct_overload.is_none() {
            // Search allowing coercions
            correct_index = 0;
            'next_func: for (binding, data) in definitions.iter().rev() {
                let ValueType::Function(func) = &data.val_type else {
                    internal_error!("");
                };

                if func.params.len() != arg_types.len() {
                    correct_index += 1;
                    continue 'next_func;
                }

                for (param_type, arg_type) in func.params.iter().zip(arg_types.iter()) {
                    if !ValueType::can_convert_type(param_type, arg_type) {
                        correct_index += 1;
                        continue 'next_func;
                    }
                }
                correct_overload = Some((binding.clone(), func.clone()));
                break;
            }
        }

        if let Some((correct_binding, func_type)) = correct_overload {
            // resolve the function according to the index
            // (the order of functions in the backlog matches the order of definitions)
            if let Some(funcs) = self
                .context
                .function_backlog
                .get_mut(identifier.lexeme().unwrap())
                && let Some(func) = funcs[correct_index].take()
            {
                self.finish_function(func);
            }
            // we know the return type now
            let ret = func_type.ret_type.clone();
            self.final_data
                .expr_types
                .insert(callee_id, ValueType::Function(func_type));
            self.final_data.bindings.insert(callee_id, correct_binding);
            ret
        } else {
            // no function that matches
            self.error_at_token(identifier, UsageError::ParamMismatch)
        }
    }

    fn update_loc(&mut self, token: &Token) {
        self.context.line = token.line();
    }
}

impl<'ast> StmtVisitor<'ast, ()> for Resolver<'ast> {
    fn visit_summon_stmt(&mut self, _path: &Vec<Token>, _alias: &Option<Token>, _id: usize) {
        todo!() // summons
    }

    fn visit_var_stmt(
        &mut self,
        name: &Token,
        explicit_type: &Option<ValueType>,
        value: &'ast Option<Box<Expr>>,
    ) {
        self.update_loc(name);

        // TODO better 'declared-no-value' semantics than this
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
            }
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
        self.context.loop_depth += 1;
        self.resolve_stmt(body);
        self.context.loop_depth -= 1;
    }

    fn visit_for_stmt(&mut self, loop_var: &Token, sequence: &Box<Expr>, body: &'ast Box<Stmt>) {
        self.update_loc(loop_var);

        self.enter_scope();
        let local_var_type = match self.resolve_expr(sequence) {
            ValueType::String => {
                self.declare_local(String::new(), ValueType::Int);
                ValueType::Char
            }
            ValueType::List(value_type) => {
                self.declare_local(String::new(), ValueType::Int);
                *value_type
            }
            ValueType::Range(t) => match *t {
                ValueType::Int => ValueType::Int, // the variable itself
                _ => self.error_at_expr(sequence, UsageError::NotIterable),
            },
            _ => self.error_at_expr(sequence, UsageError::NotIterable),
        };
        self.declare_local(loop_var.copy_ident(), local_var_type);

        self.context.loop_depth += 1;
        self.resolve_stmt(body);
        self.context.loop_depth -= 1;

        self.exit_scope();
    }

    fn visit_keyword_stmt(&mut self, keyword: &Token, arg: &Option<Box<Expr>>) {
        self.update_loc(keyword);

        match keyword.kind() {
            TokenType::Break | TokenType::Continue => {
                let mut loops_to_jump = 1;
                if let Some(arg) = arg {
                    let Expr::Literal { val, .. } = &**arg else {
                        self.error_at_expr(arg, UsageError::InvalidLoopControl);
                        return;
                    };
                    let TypedValue::Int(i) = *val else {
                        self.error_at_expr(arg, UsageError::InvalidLoopControl);
                        return;
                    };
                    if i < 1 {
                        self.error_at_expr(arg, UsageError::InvalidLoopControl);
                        return;
                    }
                    loops_to_jump = i as usize;
                }
                if loops_to_jump > self.context.loop_depth {
                    self.error_at_token(keyword, UsageError::InvalidLoopControl);
                }
            }
            TokenType::Return => {
                let ret_type = self.context.ret_type.clone();
                if let Some(arg) = arg {
                    self.expect_type(&ret_type, arg);
                } else {
                    // cheat a little bit and fake an expression
                    self.expect_type(
                        &ret_type,
                        &Expr::Literal {
                            repr: keyword.clone(),
                            val: TypedValue::None,
                            id: 0,
                        },
                    );
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
        self.update_loc(name);

        let var_name = name.copy_ident();
        let new_function_type = ValueType::func_type(ret_type, params);

        if self.in_global_scope() {
            self.declare_global(var_name, new_function_type);
        } else {
            self.declare_local(var_name, new_function_type)
        }

        // Wait for resolving until first call
        // No forward declarations in this household
        let temp = TempFunction {
            name: name.lexeme().unwrap(),
            ret_type,
            params,
            body,
        };

        let entry = self.context.function_backlog.entry(name.copy_ident());
        let value = entry.or_insert(Vec::new());
        value.push(Some(temp));
    }
}

impl<'ast> ExprVisitor<'_, ValueType> for Resolver<'ast> {
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
        ValueType::coerce_binary(&true_type, false, &false_type).unwrap_or({
            // not equal & couldn't coerce
            self.expect_resolved_type(&true_type, right, &false_type);
            ValueType::Unchecked
        }) // types are equal after this point
    }

    fn visit_binary_expr(
        &mut self,
        left: &Box<Expr>,
        op: &Token,
        right: &Box<Expr>,
        id: usize,
    ) -> ValueType {
        self.update_loc(op);

        let left = self.resolve_expr(left);
        let right = self.resolve_expr(right);
        // TODO check overloaded
        let Some(both) = ValueType::coerce_binary(&left, op.kind() == TokenType::Plus, &right)
        else {
            return self.error_at_token(op, UsageError::IncompatibleTypes);
        };
        self.final_data.bin_types.insert(id, both.clone());
        if [TokenType::DoubleEqual, TokenType::BangEqual].contains(&op.kind()) {
            // supported for all types probably
            return ValueType::Bool;
        } else if op.kind() == TokenType::DoubleDot {
            // ranges get unique logic
            return match &both {
                ValueType::Int | ValueType::Float | ValueType::Char | ValueType::String => {
                    ValueType::Range(Box::new(both))
                }
                _ => self.error_at_token(op, UsageError::InvalidRange),
            };
        }
        // after this point, both are the same type
        match both {
            ValueType::Any => {
                // TODO any in binary operators
                todo!()
            }
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
                _ => self.error_at_token(op, UsageError::InvalidOperator),
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
                _ => self.error_at_token(op, UsageError::InvalidOperator),
            },
            ValueType::Char => self.error_at_token(op, UsageError::InvalidOperator),
            ValueType::Bool => match op.kind() {
                TokenType::DoubleAmpersand | TokenType::DoubleVertBar => ValueType::Bool,
                _ => self.error_at_token(op, UsageError::InvalidOperator),
            },
            ValueType::String => match op.kind() {
                TokenType::Plus => ValueType::String,
                TokenType::Less
                | TokenType::Greater
                | TokenType::DoubleLess
                | TokenType::DoubleGreater
                | TokenType::GreaterEqual
                | TokenType::LessEqual => ValueType::Bool,
                _ => self.error_at_token(op, UsageError::InvalidOperator),
            },
            ValueType::None => self.error_at_token(op, UsageError::InvalidOperator),
            ValueType::List(value_type) => match op.kind() {
                TokenType::Plus => ValueType::String,
                TokenType::Less
                | TokenType::Greater
                | TokenType::DoubleLess
                | TokenType::DoubleGreater
                | TokenType::GreaterEqual
                | TokenType::LessEqual => ValueType::Bool,
                _ => self.error_at_token(op, UsageError::InvalidOperator),
            },
            ValueType::Object { .. } => {
                todo!() // Operator overloading?
            }
            ValueType::Range(value_type) => todo!("adding numeric ranges?"),
            ValueType::Function { .. } => panic!(),
            ValueType::Unchecked => panic!(),
        }
    }

    fn visit_assign_expr(
        &mut self,
        assignee: &Box<Expr>,
        value: &Box<Expr>,
        assign_id: usize,
    ) -> ValueType {
        let val_type = self.resolve_expr(&value);
        match &**assignee {
            Expr::Variable {
                identifier,
                id: var_id,
            } => {
                match self.get_identifier(identifier.lexeme().unwrap()) {
                    Some((binding, data)) => {
                        let data = data.clone();
                        // record id of outermost assign, not the assignee
                        self.final_data.bindings.insert(assign_id, binding);
                        self.expect_resolved_type(&data.val_type, value, &val_type);
                        self.final_data
                            .expr_types
                            .insert(*var_id, data.val_type.clone());
                        return data.val_type; // return variable type bc that's after the cast (if any)
                    }
                    None => {
                        self.error_at_expr(assignee, UsageError::UndefinedIdent);
                        val_type
                    }
                }
            }
            Expr::Get { obj, property, .. } => {
                todo!() // TODO set exprs
            }
            Expr::Slice {
                sequence, query, ..
            } => {
                let seq_type = self.resolve_expr(sequence);
                self.expect_type(&ValueType::Int, query);
                match seq_type {
                    // TODO Overload slicing for objects?
                    // maybe (a: int)[b: int] to apply bitmask?
                    // (a & (1 << b) == 1)
                    ValueType::List(item_type) => {
                        self.expect_resolved_type(&item_type, value, &val_type);
                        *item_type
                    }
                    _ => {
                        self.error_at_expr(sequence, UsageError::CantIndexThat);
                        val_type
                    }
                }
            }
            _ => {
                self.error_at_expr(&assignee, UsageError::InvalidAssign);
                val_type
            }
        }
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
        self.update_loc(op);

        let target_type = self.resolve_expr(target);
        #[cfg(test)]
        match op.kind() {
            TokenType::DoubleGreater => {
                return target_type;
            }
            TokenType::DoubleLess => {
                return ValueType::None;
            }
            _ => {}
        }
        match target_type {
            ValueType::Int => {
                // negate & bitwise not
                ValueType::Int
            }
            ValueType::Float => {
                // negate
                ValueType::Float
            }
            ValueType::Bool => {
                // logic not
                ValueType::Bool
            }
            other => {
                self.error_at_expr(target, UsageError::TypeError);
                other
            }
        }
    }

    fn visit_call_expr(&mut self, callee: &Box<Expr>, args: &Vec<Expr>, _id: usize) -> ValueType {
        if args.len() >= u8::MAX.into() {
            self.error_at_expr(args.last().unwrap(), UsageError::TooManyArgs);
        }

        let args = args.iter().collect::<Vec<&Expr>>();
        let callee = callee.as_ref();

        match callee {
            Expr::Variable { identifier, id } => {
                self.update_loc(identifier);
                self.resolve_ident_call(identifier, *id, args.as_slice())
            }
            Expr::Get { obj, property, id } => {
                self.update_loc(property);

                if args.len() + 1 >= u8::MAX.into() {
                    // im not doing CallWide, nobody needs >= 256 arguments
                    self.error_at_expr(args.last().unwrap(), UsageError::TooManyArgs);
                }

                // Pretend the object is the first argument
                let actual_args = {
                    let mut new_args = vec![obj.as_ref()];
                    new_args.extend(args.iter());
                    new_args
                };

                // resolve it as property(obj, args...)
                self.resolve_ident_call(property, *id, actual_args.as_slice())
            }
            other => {
                match self.resolve_expr(other) {
                    ValueType::Unchecked => ValueType::Unchecked,
                    ValueType::Any => todo!(),
                    ValueType::None
                    | ValueType::Int
                    | ValueType::Float
                    | ValueType::Char
                    | ValueType::Bool
                    | ValueType::String
                    | ValueType::Range(_)
                    | ValueType::List(_) => self.error_at_expr(callee, UsageError::CantCallThat),
                    ValueType::Function(func_type) => {
                        // some other expression that evaluated to a function
                        // literal/lambda?
                        let FunctionType {
                            ret_type, params, ..
                        } = *func_type;
                        if args.len() != params.len() {
                            self.error_at_expr(callee, UsageError::ParamMismatch);
                        } else {
                            for (param, arg) in params.iter().zip(args.iter()) {
                                self.expect_type(param, arg);
                            }
                        }
                        ret_type
                    }
                    ValueType::Object(obj_type) => todo!("Overload calling?"),
                }
            }
        }
    }

    fn visit_variable_expr(&mut self, identifier: &Token, id: usize) -> ValueType {
        self.update_loc(identifier);

        match self.get_identifier(identifier.lexeme().unwrap()) {
            Some((binding, data)) => {
                let mut var_type = data.val_type.clone();
                self.final_data.bindings.insert(id, binding);
                var_type
            }
            None => self.error_at_token(identifier, UsageError::UndefinedIdent),
        }
    }

    fn visit_literal_expr(&mut self, repr: &Token, val: &TypedValue, _id: usize) -> ValueType {
        self.update_loc(repr);

        self.context.value_count += 1;
        if self.context.value_count == u8::MAX as usize {
            // TODO increase max constants (maybe to u32? any more and its like. whats goin on) and have a OP_CONSTANT_WIDE
            self.error_at_token(repr, UsageError::TooManyConstants);
        }
        val.get_type()
    }

    fn visit_boolean_expr(
        &mut self,
        left: &'_ Box<Expr>,
        op: &'_ Token,
        right: &'_ Box<Expr>,
        _id: usize,
    ) -> ValueType {
        self.update_loc(op);

        self.expect_type(&ValueType::Bool, left);
        self.expect_type(&ValueType::Bool, right);
        ValueType::Bool
    }

    fn visit_slice_expr(
        &mut self,
        sequence: &'_ Box<Expr>,
        query: &'_ Box<Expr>,
        _id: usize,
    ) -> ValueType {
        let seq_type = self.resolve_expr(sequence);
        let query_type = self.resolve_expr(query);
        return match (seq_type, query_type) {
            (ValueType::String, ValueType::Int) => ValueType::Char,
            (ValueType::List(item_type), ValueType::Int) => *item_type,
            (ValueType::String, ValueType::Range(bound_type)) => {
                if *bound_type == ValueType::Int {
                    ValueType::String
                } else {
                    self.error_at_expr(query, UsageError::InvalidSlice)
                }
            }
            (ValueType::List(item_type), ValueType::Range(bound_type)) => {
                if *bound_type == ValueType::Int {
                    ValueType::List(item_type)
                } else {
                    self.error_at_expr(query, UsageError::InvalidSlice)
                }
            }
            _ => self.error_at_expr(sequence, UsageError::CantIndexThat),
        };
    }

    fn visit_get_expr(&mut self, obj: &'_ Box<Expr>, property: &'_ Token, id: usize) -> ValueType {
        self.update_loc(property);
        let property_str = property.lexeme().unwrap().as_str();

        match self.resolve_expr(obj) {
            ValueType::String => match property_str {
                "len" => return ValueType::Int,
                _ => self.error_at_token(property, UsageError::NoSuchField),
            },
            ValueType::List(_) => match property_str {
                "len" => ValueType::Int,
                _ => self.error_at_token(property, UsageError::NoSuchField),
            },
            ValueType::Range(value_type) => match property_str {
                "start" | "end" => *value_type,
                _ => self.error_at_token(property, UsageError::NoSuchField),
            },
            ValueType::Object(object_type) => todo!(),
            _ => self.error_at_expr(obj, UsageError::DontGotFields),
        }
    }

    fn visit_list_expr(&mut self, items: &'_ Vec<Expr>, _id: usize) -> ValueType {
        if items.is_empty() {
            return ValueType::List(Box::new(ValueType::Unchecked));
        }
        let first_type = self.resolve_expr(&items[0]);
        // TODO weird type expectations
        // [1, 2.0] fails but [1.0, 2] is fine?
        for item in &items[1..] {
            if !self.expect_type(&first_type, item) {
                return ValueType::List(Box::new(ValueType::Unchecked));
            }
        }
        ValueType::List(Box::new(first_type))
    }
}
