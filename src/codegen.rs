use std::{
    collections::HashMap,
    rc::Rc,
};

use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::{
    analysis::{AnalysisResult, Binding}, debug::opcodes::disassemble, expr_ast::{Expr, ExprVisitor}, lexing::{Token, TokenType}, stmt_ast::{Stmt, StmtVisitor}, values::{Function, Object, ObjectType, Value, ValueType}
};

#[derive(Debug, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum OpCode {
    Pass = 0x80, // to avoid normal arguments being parsed as bytecode
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
    BoolAnd,
    BoolOr,
}

/// Returns a function to run.
pub fn generate(ast: &Vec<Stmt>, context: AnalysisResult) -> Function {
    let AnalysisResult {
        bindings,
        bin_types,
        expr_types,
    } = context;
    let mut generator = Generator {
        var_counts: Vec::new(),
        bindings,
        bin_types,
        expr_types,
        is_main: true,
        function: TempFunction::new(String::new(), Box::new([]), ValueType::Bool),
    };
    generator.var_counts.push(0);
    for stmt in ast {
        generator.code_stmt(stmt);
    }
    Function::from(generator.function)
}

struct Generator {
    bindings: HashMap<usize, Binding>,
    bin_types: HashMap<usize, ValueType>,
    expr_types: HashMap<usize, ValueType>,
    
    var_counts: Vec<u8>,
    is_main: bool,
    function: TempFunction,
}

struct TempFunction {
    name: String,
    params: Box<[ValueType]>,
    ret_type: ValueType,
    constants: Vec<Value>,
    code: Vec<u8>,
}

impl TempFunction {
    fn new(name: String, params: Box<[ValueType]>, ret_type: ValueType) -> Self {
        Self {
            name,
            params,
            ret_type,
            constants: Vec::new(),
            code: Vec::new(),
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
        } = value;
        let new = Self {
            name, params, ret_type,
            constants: constants.into_boxed_slice(),
            code: code.into_boxed_slice(),
        };
        #[cfg(feature = "debug")]
        disassemble(&new);
        new
    }
}

impl Generator {
    fn code_stmt(&mut self, stmt: &Stmt) {
        stmt.accept(self)
    }
    fn code_expr(&mut self, expr: &Expr) {
        expr.accept(self)
    }

    fn constant(&mut self, value: &Value) {
        match value {
            Value::Bool(b) => self.write_instr(if *b { OpCode::True } else { OpCode::False }),
            Value::None => self.write_instr(OpCode::None),
            _ => {
                self.write_instr(OpCode::Constant);
                let index = self.function.constants.len();
                self.write_byte(index as u8);
                self.function.constants.push(value.clone());
            }
        }
    }

    fn write_instr(&mut self, instr: OpCode) {
        self.function.code.push(instr.into());
    }

    /// Returns the index of the new byte.
    fn write_byte(&mut self, byte: u8) -> usize {
        self.function.code.push(byte);
        self.function.code.len() - 1
    }
    
    /// Returns the index of the first byte in the new short.
    fn write_short(&mut self, short: u16) -> usize {
        self.write_byte((short >> 8) as u8);   // most significant
        self.write_byte((short & 0xff) as u8); // least significant
        self.function.code.len() - 2
    }

    /// Returns the ip after reading the full jump instruction.
    fn write_jump(&mut self, instr: OpCode) -> usize {
        self.write_instr(instr);
        self.write_short(0);
        self.function.code.len() - 2
    }

    // jump_loc = ip after parsing a jump
    fn patch_jump_to_next(&mut self, jump_loc: usize) {
        let offset = (self.function.code.len() as isize) - (jump_loc as isize) - 2;
        // I HATE OFF-BY-ONE ERRORS!!!
        if i16::MIN as isize <= offset && offset <= i16::MAX as isize {
            self.function.code[ jump_loc ] = (offset >> 8) as u8; // arg byte 1
            self.function.code[jump_loc+1] = (offset & 0xff) as u8; // arg byte 2
        } else {
            todo!("JumpLong?")
        }
    }

    fn in_global_scope(&self) -> bool {
        self.is_main && self.var_counts.len() == 1
    }

    fn take_type(&mut self, expr: &Expr) -> ValueType {
        self.expr_types.remove(&expr.id())
            .expect("Expression type should be present")
    }
}

impl StmtVisitor<'_, ()> for Generator {
    fn visit_function_stmt(
        &mut self,
        ret_type: &ValueType,
        name: &Token,
        params: &Vec<(Token, ValueType)>,
        body: &Vec<Stmt>,
        id: usize,
    ) -> () {
        use std::mem::replace;

        let new_func = TempFunction::new(
            name.copy_ident(),
            params.iter()
                .map(|(_, tp)| tp.clone())
                .collect::<Vec<ValueType>>()
                .into_boxed_slice(),
            ret_type.clone());

        let old_func = replace(&mut self.function, new_func);
        let old_counts = replace(&mut self.var_counts, Vec::new());
        let old_status = self.is_main;

        self.is_main = false;
        for stmt in body {
            self.code_stmt(stmt);
        }
        self.write_instr(OpCode::None);
        self.write_instr(OpCode::Return);

        let func = Function::from(replace(&mut self.function, TempFunction::dummy()));

        self.function = old_func;
        self.var_counts = old_counts;
        self.is_main = old_status;
        
        self.constant(&Value::Object(Rc::new(Object::Function(func))));
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
        todo!() // TODO
    }

    fn visit_var_stmt(&mut self, _name: &Token, val: &Option<Box<Expr>>) -> () {
        if self.in_global_scope() {
            self.code_expr(val.as_ref().unwrap());
            self.write_instr(OpCode::DefineGlobal);
        } else {
            if let Some(expr) = val {
                self.code_expr(expr);
            } else {
                self.write_instr(OpCode::None);
            }
        }
    }

    fn visit_block_stmt(&mut self, statements: &Vec<Stmt>) -> () {
        self.var_counts.push(0);
        for stmt in statements {
            self.code_stmt(stmt);
        }
        for _ in 0..self.var_counts.pop().unwrap() {
            // pop locals
            self.write_instr(OpCode::Pop);
        }
    }

    fn visit_expression_stmt(&mut self, expression: &Box<Expr>) -> () {
        self.code_expr(expression);
        self.write_instr(OpCode::Pop);
    }

    fn visit_if_stmt(
        &mut self,
        condition: &Box<Expr>,
        true_branch: &Box<Stmt>,
        false_branch: &Option<Box<Stmt>>,
    ) -> () {
        todo!() // TODO
    }

    fn visit_while_stmt(&mut self, condition: &Box<Expr>, body: &Box<Stmt>) -> () {
        todo!() // TODO
    }

    fn visit_for_stmt(
        &mut self,
        var: &Token,
        sequence: &Box<Expr>,
        body: &Box<Stmt>,
    ) -> () {
        todo!() // TODO
    }

    fn visit_keyword_stmt(&mut self, keyword: &Token, arg: &Option<Box<Expr>>) -> () {
        match keyword.kind() {
            TokenType::Break => todo!(), // TODO
            TokenType::Continue => todo!(), // TODO
            TokenType::Return => {
                match arg {
                    Some(arg) => self.code_expr(arg),
                    None => self.write_instr(OpCode::None),
                }
                self.write_instr(OpCode::Return);
            },
            _ => panic!()
        }
    }
}

impl ExprVisitor<'_, ()> for Generator {
    fn visit_conditional_expr(&mut self, condition: &Box<Expr>, if_true: &Box<Expr>, if_false: &Box<Expr>, id: usize) -> () {
        self.code_expr(condition);
        let false_jump = self.write_jump(OpCode::JumpIfNot); // condition popped here
        self.code_expr(if_true);
        let end_jump = self.write_jump(OpCode::Jump); // jump over false branch
        self.patch_jump_to_next(false_jump);
        self.code_expr(if_false);
        self.patch_jump_to_next(end_jump);
    }

    fn visit_binary_expr(&mut self, left: &Box<Expr>, op: &Token, right: &Box<Expr>, id: usize) -> () {
        let both = self.bin_types.remove(&id).expect("Should have resolved types in binary");
        self.code_expr(left);
        // TODO cast if needed
        if self.take_type(left) != both {
            todo!()
        }
        self.code_expr(right);
        if self.take_type(right) != both {
            todo!()
        }
        
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
            ValueType::Any => todo!(), // TODO
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
                TokenType::DoubleDot => todo!(), // TODO
                _ => panic!("Invalid operator made it to codegen")
            },
            ValueType::Char => todo!(), // TODO
            ValueType::Bool => match op.kind() {
                TokenType::DoubleAmpersand => self.write_instr(OpCode::BoolAnd),
                TokenType::DoubleVertBar => self.write_instr(OpCode::BoolOr),
                _ => panic!("Invalid operator made it to codegen")
            }
            ValueType::Object(obj_type) => match obj_type {
                ObjectType::String => todo!(), // TODO
                _ => panic!("Invalid type made it to codegen")
            },
            ValueType::None => panic!("Invalid type made it to codegen")
        }
    }

    fn visit_assign_expr(&mut self, _assignee: &Box<Expr>, value: &Box<Expr>, id: usize) -> () {
        // TODO other valid assignees (get, index)
        self.code_expr(value);
        let loc = self.bindings.get(&id).expect("Assign expr should be bound.");
        match loc.clone() {
            Binding::Stack(index) => todo!(), // TODO
            Binding::Globals(index) => {
                self.write_instr(OpCode::SetGlobal);
                self.write_byte(index as u8);
            },
        }
    }

    fn visit_cast_expr(&mut self,
        expr: &Box<Expr>, new_type: &ValueType, id: usize) -> () {
        todo!() // TODO
    }

    fn visit_unary_expr(&mut self, op: &Token, target: &Box<Expr>, prefix: &bool, id: usize) -> () {
        self.code_expr(target);
        match self.take_type(target) {
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
        callee: &Box<Expr>, args: &Vec<Expr>, id: usize) -> () {
        todo!() // TODO
    }

    fn visit_variable_expr(&mut self, _identifier: &Token, id: usize) -> () {
        match self.bindings.remove(&id).expect("Variable expr should be bound") {
            Binding::Stack(index) => {
                self.write_instr(OpCode::GetGlobal);
                self.write_byte(index as u8);
            }
            Binding::Globals(index) => {
                self.write_instr(OpCode::GetGlobal);
                self.write_byte(index as u8);
            }
        }
    }

    fn visit_literal_expr(&mut self,
        repr: &Token, val: &Value, id: usize) -> () {
        self.constant(val);
    }
}
