use std::{
    collections::HashMap,
    rc::Rc,
};

use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::{
    analysis::{AnalysisResult, Binding}, builtins::STR_FUNC_INDEX, debug::opcodes::disassemble, expr_ast::{Expr, ExprVisitor}, lexing::{Token, TokenType}, stmt_ast::{Stmt, StmtVisitor}, types::*, values::*
};

#[derive(Debug, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum OpCode {
    Pass = 0x80,
    None,
    True,
    False,
    Pop,
    Return,
    Constant, // [const pool index]
    GetLocal, // [stack index]
    SetLocal, // [stack index]
    GetGlobal, // [globals index]
    SetGlobal, // [globals index]
    DefineGlobal,
    Jump, // [ip offset][byte 2 of short]
    JumpIfNot, // [ip offset][byte 2]
    Call, // # of arguments to parse
    SwapTop, // [stack index from end, 0 == len-1]
    
    // ========== Casts ==========
    IntToFloat,
    IntToBool,
    CharToInt,
    BoolToInt,
    BoolToFloat,
    WrapAny,
    AnyToInt,
    AnyToFloat,
    AnyToBool,
    AnyToChar,
    AnyToString,
    ToStringTEMP,

    // ========== Arithmetic operators ==========

    ValEqual,
    Concat,

    // Floats
    FloatAdd,
    FloatSub,
    FloatMul,
    FloatDiv,
    FloatMod,
    FloatNegate,
    FloatLess,
    FloatGreater,

    // Ints
    IntAdd,
    IntSub,
    IntMul,
    IntDiv,
    IntMod,
    IntAnd,
    IntXor,
    IntOr,
    IntShl,
    IntShr,
    IntNegate,
    IntNot,
    IntLess,
    IntGreater,

    // Bools
    BoolNot,

    Crash,
}

/// Returns a function to run.
pub fn generate(ast: &Vec<Stmt>, context: AnalysisResult) -> Function {
    // All semantics-related restrictions are checked during analysis,
    // so by this point, the program is expected to be valid.
    // Expect to see lots of .unwrap()s and _ => panic!()s throughout.
    let AnalysisResult {
        bindings,
        bin_types,
        expr_types,
    } = context;
    let mut generator = Generator {
        bindings,
        bin_types,
        expr_types,
        context: FunctionContext::new(
            TempFunction::new(
                String::new(),
                Box::new([]), // string[] argv?
                ValueType::None),
            true),
    };
    generator.context.var_counts.push(0);
    for stmt in ast {
        generator.code_stmt(stmt);
    }
    generator.write_instr(OpCode::None);
    generator.write_instr(OpCode::Return);
    Function::from(generator.context.function)
}

struct Generator {
    // global
    bindings: HashMap<usize, Binding>,
    bin_types: HashMap<usize, ValueType>,
    expr_types: HashMap<usize, ValueType>,
    
    context: FunctionContext,
}

struct FunctionContext {
    var_counts: Vec<u8>,
    loop_starts: Vec<usize>, // byte indices
    break_backlog: Vec<(usize, i64)>, // jumps index & loops to break
    is_main: bool,
    function: TempFunction,
    prev_line: u32,
}

impl FunctionContext {
    fn new(function: TempFunction, is_main: bool) -> Self {
        Self {
            var_counts: Vec::new(),
            loop_starts: Vec::new(),
            break_backlog: Vec::new(),
            is_main,
            function,
            prev_line: 0
        }
    }
}

struct TempFunction {
    name: String,
    params: Box<[ValueType]>,
    ret_type: ValueType,
    constants: Vec<Value>,
    code: Vec<u8>,
    lines: Vec<LineRLE>
}

#[derive(Debug, PartialEq)]
pub struct LineRLE {
    pub line: u32,
    pub count: u32
}

impl TempFunction {
    fn new(name: String, params: Box<[ValueType]>, ret_type: ValueType) -> Self {
        Self {
            name,
            params,
            ret_type,
            constants: Vec::new(),
            code: Vec::new(),
            lines: Vec::new(),
        }
    }

    fn dummy() -> Self {
        Self::new(String::new(), Box::new([]), ValueType::None)
    }
}

impl From<TempFunction> for Function {
    fn from(value: TempFunction) -> Self {
        let TempFunction {
            name,
            params,
            ret_type,
            constants,
            code,
            lines,
        } = value;
        let new = Self {
            name, params, ret_type,
            constants: constants.into_boxed_slice(),
            code: code.into_boxed_slice(),
            lines: lines.into_boxed_slice(),
        };
        #[cfg(feature = "debug")]
        disassemble(&new);
        new
    }
}

impl Generator {

    fn cast_instr(old_type: &ValueType, new_type: &ValueType) -> Option<OpCode> {
        if *old_type == ValueType::Any {
            match new_type {
                ValueType::Int => Some(OpCode::AnyToInt),
                ValueType::Float => Some(OpCode::AnyToFloat),
                ValueType::Char => Some(OpCode::AnyToChar),
                ValueType::Bool => Some(OpCode::AnyToBool),
                ValueType::String => Some(OpCode::AnyToString),
                _ => None
            }
        } else if *new_type == ValueType::Any {
            Some(OpCode::WrapAny)
        } else {
            match (&old_type, new_type) {
                (ValueType::Int, ValueType::Float) => Some(OpCode::IntToFloat),
                (ValueType::Int, ValueType::Bool) => Some(OpCode::IntToBool),
                (ValueType::Char, ValueType::Int) => Some(OpCode::CharToInt),
                (ValueType::Bool, ValueType::Int) => Some(OpCode::BoolToInt),
                (ValueType::Bool, ValueType::Float) => Some(OpCode::BoolToFloat),
                _ => None
            }
        }
    }

    fn code_stmt(&mut self, stmt: &Stmt) {
        stmt.accept(self)
    }

    fn code_expr_as_is(&mut self, expr: &Expr) {
        expr.accept(self)
    } 

    fn update_loc(&mut self, token: &Token) {
        self.context.prev_line = token.line();
    }

    fn constant(&mut self, value: Value) {
        match value {
            Value::Bool(b) => self.write_instr(if b { OpCode::True } else { OpCode::False }),
            Value::None => self.write_instr(OpCode::None),
            _ => {
                self.write_instr(OpCode::Constant);
                let index = self.context.function.constants.len();
                self.write_byte(index as u8);
                self.context.function.constants.push(value);
            }
        }
    }

    fn write_instr(&mut self, instr: OpCode) {
        self.write_byte(instr.into());
    }

    /// Returns the index of the new byte.
    fn write_byte(&mut self, byte: u8) -> usize {
        self.context.function.code.push(byte);
        self.update_lines();
        self.context.function.code.len() - 1
    }

    fn update_lines(&mut self) {
        let cur_line = self.context.prev_line;
        let lines = &mut self.context.function.lines;
        let rle_end = lines.last_mut();
        if let Some(last) = rle_end && last.line == cur_line {
            last.count += 1;
        } else {
            lines.push(LineRLE { line: cur_line, count: 1 });
            return;
        }
    }
    
    /// Returns the index of the first byte in the new short.
    fn write_short(&mut self, short: u16) -> usize {
        self.write_byte((short >> 8) as u8);   // most significant
        self.write_byte((short & 0xff) as u8); // least significant
        self.context.function.code.len() - 2
    }

    /// Returns the ip after reading the full jump instruction.
    fn write_jump(&mut self, instr: OpCode) -> usize {
        self.write_instr(instr);
        self.write_short(0);
        self.context.function.code.len() - 2
    }

    // jump_loc = ip after parsing a jump
    fn patch_jump_to_next(&mut self, jump_arg_index: usize) {
        //                                                          pointing at idx of arg + 2 at time of jump
        let offset = (self.context.function.code.len() as isize) - (jump_arg_index as isize + 2);
        if i16::MIN as isize <= offset && offset <= i16::MAX as isize {
            self.context.function.code[ jump_arg_index ] = (offset >> 8) as u8; // arg byte 1
            self.context.function.code[jump_arg_index+1] = (offset & 0xff) as u8; // arg byte 2
        } else {
            todo!("JumpLong with i32?")
        }
    }

    fn write_jump_back(&mut self, new_loc: usize) {
        //                                  current len + 1 instr byte + 2 arg bytes = 3 at time of jump
        let offset = (new_loc as isize) - (self.context.function.code.len() as isize + 3);
        if i16::MIN as isize <= offset && offset <= i16::MAX as isize {
            self.write_instr(OpCode::Jump);
            self.write_short((offset as i16).cast_unsigned());
        } else {
            todo!("JumpLong? but backwards")
        }
    }

    fn in_global_scope(&self) -> bool {
        self.context.is_main && self.context.var_counts.len() == 1
    }

    fn take_expr_type(&mut self, expr: &Expr) -> ValueType {
        self.expr_types.remove(&expr.id())
            .expect("Expression type should be present")
    }

    /// Tells `continue`s where to jump
    fn begin_loop(&mut self, loop_start: usize) {
        self.context.loop_starts.push(loop_start);
    }

    /// Uses the current length as the loop end index,
    /// which is the 1st instruction after the loop body.
    fn end_loop(&mut self) {
        self.context.loop_starts.pop();
        let loop_ended = self.context.loop_starts.len();
        let mut backlog = self.context.break_backlog.clone();
        backlog.retain(|(jump, loop_broken)|
            if *loop_broken as usize == loop_ended {
                self.patch_jump_to_next(*jump);
                false
            } else {
                true
            }
        );
        self.context.break_backlog = backlog;
    }

    /// Codes an expression, casting or converting to string if needed.
    /// Panics otherwise, as invalid coercions should be caught during analysis.
    fn code_expr_with_cast(&mut self, expected: &ValueType, expr: &Expr) {
        let expr_type = self.take_expr_type(expr);
        if expr_type == *expected {
            self.code_expr_as_is(expr);
            return;
        }

        // Cast is needed
        if let Some(code) = Self::cast_instr(&expr_type, expected) {
            // Got a corresponding cast instruction from expr_type to expected
            self.code_expr_as_is(expr);
            self.write_instr(code);
        } else if *expected == ValueType::String {
            // No string cast, so we call __str instead
            self.write_instr(OpCode::GetGlobal);
            self.write_byte(STR_FUNC_INDEX);
            self.code_expr_as_is(expr);
            self.write_instr(OpCode::Call);
            self.write_byte(1);
        } else {
            panic!("Unhandled coercion");
        }
    }
}

impl StmtVisitor<'_, ()> for Generator {
    fn visit_function_stmt(
        &mut self,
        ret_type: &ValueType,
        name: &Token,
        params: &Vec<(Token, ValueType)>,
        body: &Vec<Stmt>,
        _id: usize,
    ) -> () {
        use std::mem::replace;

        self.update_loc(name);

        let new_func = TempFunction::new(
            name.copy_ident(),
            params.iter()
                .map(|(_, tp)| tp.clone())
                .collect::<Vec<ValueType>>()
                .into_boxed_slice(),
            ret_type.clone());

        let old_context = std::mem::replace(
            &mut self.context, FunctionContext::new(new_func, false));

        for stmt in body {
            self.code_stmt(stmt);
        }

        // This extra check is more to reduce visual noise over saving two bytes
        if let Some(&last) = self.context.function.code.last()
            && last == OpCode::Return.into() {} else {
            // Last instruction wasn't return, so we return a dummy value from each type
            // TODO ensure all control flow paths return a valid value instead of this
            let dummy = ValueType::dummy(&self.context.function.ret_type);
            self.constant(dummy);
            self.write_instr(OpCode::Return);
        }

        let func = Function::from(replace(&mut self.context.function, TempFunction::dummy()));

        self.context = old_context;
        
        // Store new function in the heap and register
        // a pointer to it in the outer function's constants
        self.constant(Value::Function(Rc::new(func)));
        if self.in_global_scope() {
            self.write_instr(OpCode::DefineGlobal);
        }
    }

    fn visit_summon_stmt(
        &mut self,
        path: &Vec<Token>,
        alias: &Option<Token>,
        id: usize,
    ) -> () {
        todo!() // TODO summons
    }

    fn visit_var_stmt(&mut self, name: &Token, var_type: &Option<ValueType>, val: &Option<Box<Expr>>) -> () {
        // new T x = val -> new T x = val as T
        // new T x       -> new T x = T.dummy() // created during compile time
        // new x = val   -> new [val.type()] x = val
        // new x         -> new any x = none as any
        // TODO write this down somewhere

        self.update_loc(name);

        match (var_type, val) {
            (None, None) => {
                self.write_instr(OpCode::None);
                self.write_instr(OpCode::WrapAny);
            },
            (None, Some(val)) => self.code_expr_as_is(val),
            (Some(var), None) => 
                self.constant(var.dummy()),
            (Some(var), Some(val)) =>
                self.code_expr_with_cast(var, val),
        }

        if self.in_global_scope() {
            self.write_instr(OpCode::DefineGlobal);
        }
    }

    fn visit_block_stmt(&mut self, statements: &Vec<Stmt>) -> () {
        self.context.var_counts.push(0);
        for stmt in statements {
            self.code_stmt(stmt);
        }
        for _ in 0..self.context.var_counts.pop().unwrap() {
            // pop locals
            self.write_instr(OpCode::Pop);
        }
    }

    fn visit_expression_stmt(&mut self, expression: &Box<Expr>) -> () {
        self.code_expr_as_is(expression);
        self.write_instr(OpCode::Pop);
    }

    fn visit_if_stmt(
        &mut self,
        condition: &Box<Expr>,
        true_branch: &Box<Stmt>,
        false_branch: &Option<Box<Stmt>>,
    ) -> () {
        self.code_expr_with_cast(&ValueType::Bool, condition);
        let else_jump = self.write_jump(OpCode::JumpIfNot);
        self.code_stmt(true_branch);
        let mut end_jump = 0;
        if false_branch.is_some() {
            end_jump = self.write_jump(OpCode::Jump);
        }
        self.patch_jump_to_next(else_jump);
        if let Some(false_branch) = false_branch {
            self.code_stmt(false_branch);
            self.patch_jump_to_next(end_jump);
        }
    }

    fn visit_while_stmt(&mut self, condition: &Box<Expr>, body: &Box<Stmt>) -> () {
        let start = self.context.function.code.len();
        self.begin_loop(start);
        self.code_expr_with_cast(&ValueType::Bool, condition);
        let end = self.write_jump(OpCode::JumpIfNot);
        self.code_stmt(body);
        self.write_jump_back(start);
        self.patch_jump_to_next(end);
        self.end_loop();
    }

    fn visit_for_stmt(
        &mut self,
        var: &Token,
        sequence: &Box<Expr>,
        body: &Box<Stmt>,
    ) -> () {
        self.update_loc(var);

        let start = self.context.function.code.len();
        self.begin_loop(start);
        todo!();
        // self.end_loop();
    }

    fn visit_keyword_stmt(&mut self, keyword: &Token, arg: &Option<Box<Expr>>) -> () {
        self.update_loc(keyword);

        match keyword.kind() {
            TokenType::Break => {
                let mut loops_to_jump = 1;
                if let Some(arg) = arg {
                    // we already know this is a valid integer
                    let Expr::Literal { val, .. } = &**arg else { panic!() };
                    let Value::Int(i) = val else { panic!() };
                    loops_to_jump = *i as usize;
                }
                let jump = self.write_jump(OpCode::Jump);
                let index_of_loop_broken = self.context.loop_starts.len() - loops_to_jump;
                /* Consider:
                while x { // loop 0
                    while y { // loop 1
                        while z { // loop 2
                            // The "loop depth" at this point is 3.
                            // This break statement would jump to the end of loop (3 - 2 = 1).
                            break 2
                            break 1 // to loop 3 - 1 = 2 end
                            break 3 // to loop 3 - 3 = 0 end
                        }
                        // loop 2 end
                    }
                    // loop 1 end
                }
                // loop 0 end
                 */
                self.context.break_backlog.push((jump, index_of_loop_broken as i64));
            }
            TokenType::Continue => {
                let mut loops_to_jump = 1;
                if let Some(arg) = arg {
                    // we already know this is a valid integer
                    let Expr::Literal { val, .. } = &**arg else { panic!() };
                    let Value::Int(i) = val else { panic!() };
                    loops_to_jump = *i as usize;
                }
                let nth_loop_start = self.context.loop_starts[self.context.loop_starts.len() - loops_to_jump];
                self.write_jump_back(nth_loop_start);
            }
            TokenType::Return => {
                match arg {
                    Some(arg) => {
                        let ret_type = self.context.function.ret_type.clone();
                        self.code_expr_with_cast(&ret_type, arg);
                    }
                    None => self.write_instr(OpCode::None),
                }
                self.write_instr(OpCode::Return);
            },
            _ => panic!()
        }
    }
}

impl ExprVisitor<'_, ()> for Generator {
    fn visit_conditional_expr(&mut self, condition: &Box<Expr>, if_true: &Box<Expr>, if_false: &Box<Expr>, _id: usize) -> () {
        self.code_expr_as_is(condition);
        let false_jump = self.write_jump(OpCode::JumpIfNot); // condition popped here
        self.code_expr_as_is(if_true);
        let end_jump = self.write_jump(OpCode::Jump); // jump over false branch
        self.patch_jump_to_next(false_jump);
        self.code_expr_as_is(if_false);
        self.patch_jump_to_next(end_jump);
    }

    fn visit_binary_expr(&mut self, left: &Box<Expr>, op: &Token, right: &Box<Expr>, id: usize) -> () {
        self.update_loc(op);

        let both = self.bin_types.remove(&id).expect("Should have resolved types in binary");
        self.code_expr_with_cast(&both, left);
        self.code_expr_with_cast(&both, right);
        
        match op.kind() {
            TokenType::DoubleEqual => {
                self.write_instr(OpCode::ValEqual);
                return;
            }
            TokenType::BangEqual => {
                self.write_instr(OpCode::ValEqual);
                self.write_instr(OpCode::BoolNot);
                return;
            }
            _ => {}
        }
        
        match both {
            ValueType::Any => todo!(), // TODO `any` operators
            ValueType::Int => match op.kind() {
                TokenType::Plus => self.write_instr(OpCode::IntAdd),
                TokenType::Minus => self.write_instr(OpCode::IntSub),
                TokenType::Star => self.write_instr(OpCode::IntMul),
                TokenType::FSlash => self.write_instr(OpCode::IntDiv),
                TokenType::Percent => self.write_instr(OpCode::IntMod),
                TokenType::Ampersand => self.write_instr(OpCode::IntAnd),
                TokenType::Carrot => self.write_instr(OpCode::IntXor),
                TokenType::VertBar => self.write_instr(OpCode::IntOr),
                TokenType::DoubleLess => self.write_instr(OpCode::IntShl),
                TokenType::DoubleGreater => self.write_instr(OpCode::IntShr),
                TokenType::Greater => self.write_instr(OpCode::IntGreater),
                TokenType::Less => self.write_instr(OpCode::IntLess),
                TokenType::GreaterEqual => {
                    self.write_instr(OpCode::IntLess);
                    self.write_instr(OpCode::BoolNot);
                },
                TokenType::LessEqual => {
                    self.write_instr(OpCode::IntGreater);
                    self.write_instr(OpCode::BoolNot);
                },
                TokenType::DoubleDot => todo!(),
                _ => panic!("Invalid operator made it to codegen")
            },
            ValueType::Float => match op.kind() {
                TokenType::Plus => self.write_instr(OpCode::FloatAdd),
                TokenType::Minus => self.write_instr(OpCode::FloatSub),
                TokenType::Star => self.write_instr(OpCode::FloatMul),
                TokenType::FSlash => self.write_instr(OpCode::FloatDiv),
                TokenType::Percent => self.write_instr(OpCode::FloatMod),
                TokenType::Less => self.write_instr(OpCode::FloatLess),
                TokenType::Greater => self.write_instr(OpCode::FloatGreater),
                TokenType::GreaterEqual => {
                    self.write_instr(OpCode::FloatLess);
                    self.write_instr(OpCode::BoolNot);
                },
                TokenType::LessEqual => {
                    self.write_instr(OpCode::FloatGreater);
                    self.write_instr(OpCode::BoolNot);
                },
                TokenType::DoubleDot => todo!(),
                _ => panic!("Invalid operator made it to codegen")
            },
            ValueType::Bool => panic!("Boolean operands should be in Expr::Logical"),
            ValueType::String => match op.kind() {
                TokenType::Plus => self.write_instr(OpCode::Concat),
                TokenType::Less => self.write_instr(OpCode::FloatLess),
                TokenType::Greater => self.write_instr(OpCode::FloatGreater),
                TokenType::GreaterEqual => {
                    self.write_instr(OpCode::FloatLess);
                    self.write_instr(OpCode::BoolNot);
                },
                TokenType::LessEqual => {
                    self.write_instr(OpCode::FloatGreater);
                    self.write_instr(OpCode::BoolNot);
                },
                _ => panic!("Invalid operator made it to codegen")
            }
            _ => panic!("Invalid type made it to codegen")
        }
    }

    fn visit_assign_expr(&mut self, assignee: &Box<Expr>, value: &Box<Expr>, id: usize) -> () {
        let assignee_type = self.expr_types.remove(&assignee.id())
            .expect("Assignee type should be recorded during analysis");
        self.code_expr_with_cast(&assignee_type, value);
        match &**assignee {
            Expr::Slice { sequence, query, .. } => {
                self.code_expr_as_is(sequence);
                self.code_expr_as_is(query);
                todo!()
            }
            Expr::Get { obj, id, .. } => {
                self.code_expr_as_is(obj);
                todo!()
            }
            Expr::Variable { .. } => {
                // use id of outermost assign, not the assignee
                let loc = self.bindings.get(&id).expect("Assign expr should be bound.");
                match loc.clone() {
                    Binding::Stack(index) => {
                        self.write_instr(OpCode::SetLocal);
                        self.write_byte(index as u8);
                    },
                    Binding::Globals(index) => {
                        self.write_instr(OpCode::SetGlobal);
                        self.write_byte(index as u8);
                    },
                }
            }
            _ => panic!("Invalid assign target made it to codegen")
        }
    }

    fn visit_cast_expr(&mut self,
        expr: &Box<Expr>, new_type: &ValueType, id: usize) -> () {
        self.code_expr_as_is(expr);
        let old_type = self.take_expr_type(expr);
        if old_type == *new_type {
            return;
        }
        let cast_byte = Self::cast_instr(&old_type, new_type)
            .expect("Invalid cast should be rejected during analysis");
        self.write_instr(cast_byte);
    }

    fn visit_unary_expr(&mut self, op: &Token, target: &Box<Expr>, prefix: &bool, id: usize) -> () {
        self.update_loc(op);

        self.code_expr_as_is(target);
        match self.take_expr_type(target) {
            ValueType::Int => match op.kind() {
                TokenType::Tilde => self.write_instr(OpCode::IntNot),
                TokenType::Minus => self.write_instr(OpCode::IntNegate),
                _ => panic!("Invalid operator made it to codegen")
            },
            ValueType::Float => match op.kind() {
                TokenType::Minus => self.write_instr(OpCode::FloatNegate),
                _ => panic!("Invalid operator made it to codegen")
            },
            ValueType::Bool => match op.kind() {
                TokenType::Bang => self.write_instr(OpCode::BoolNot),
                _ => panic!("Invalid operator made it to codegen")
            },
            _ => panic!("Invalid type made it to codegen")
        }
    }

    fn visit_call_expr(&mut self,
        callee: &Box<Expr>, args: &Vec<Expr>, _id: usize) -> () {
        self.code_expr_as_is(callee);
        let params;
        match self.take_expr_type(callee) {
            ValueType::Function { params: callee_params, .. } => {
                params = callee_params;
            },
            _ => panic!()
        }
        // We know the arg & param length are the same
        for (arg, param) in args.iter().zip(params.iter()) {
            self.code_expr_with_cast(param, arg);
        }
        self.write_instr(OpCode::Call);
        self.write_byte(args.len() as u8);
    }

    fn visit_variable_expr(&mut self, identifier: &Token, id: usize) -> () {
        self.update_loc(identifier);

        match self.bindings.remove(&id).expect("Variable expr should be bound") {
            Binding::Stack(index) => {
                self.write_instr(OpCode::GetLocal);
                self.write_byte(index as u8);
            }
            Binding::Globals(index) => {
                self.write_instr(OpCode::GetGlobal);
                self.write_byte(index as u8);
            }
        }
    }

    fn visit_literal_expr(&mut self,
        repr: &Token, val: &Value, _id: usize) -> () {
        self.update_loc(repr);

        self.constant(val.clone());
    }
    
    fn visit_boolean_expr(&mut self,
        left: &'_ Box<Expr>, op: &'_ Token, right: &'_ Box<Expr>, _id: usize) -> () {
        self.code_expr_with_cast(&ValueType::Bool, left);
        match op.kind() {
            TokenType::DoubleAmpersand => {
                // a and b == if [a]: [b], else [false]
                let skip_right = self.write_jump(OpCode::JumpIfNot);
                // left is true, push right
                self.code_expr_with_cast(&ValueType::Bool, right);
                let end_jump = self.write_jump(OpCode::Jump);
                self.patch_jump_to_next(skip_right);
                // left is false, jump to false
                self.write_instr(OpCode::False);
                self.patch_jump_to_next(end_jump);
            }
            TokenType::DoubleVertBar => {
                // a or b == if [a]: [true], else [b]
                let goto_right = self.write_jump(OpCode::JumpIfNot);
                // left is true, push true
                self.write_instr(OpCode::True);
                let end_jump = self.write_jump(OpCode::Jump);
                self.patch_jump_to_next(goto_right);
                // left is false, jump to right
                self.code_expr_with_cast(&ValueType::Bool, right);
                self.patch_jump_to_next(end_jump);
            }
            _ => panic!("Invalid operator made it to codegen")
        }
    }
    
    fn visit_slice_expr(&mut self,
        sequence: &'_ Box<Expr>, query: &'_ Box<Expr>, id: usize) -> () {
        todo!()
    }
    
    fn visit_method_expr(&mut self,
        obj: &'_ Box<Expr>, method: &'_ Token, args: &'_ Vec<Expr>, id: usize) -> () {
        todo!()
    }
    
    fn visit_get_expr(&mut self,
        obj: &'_ Box<Expr>, property: &'_ Token, id: usize) -> () {
        todo!()
    }
}
