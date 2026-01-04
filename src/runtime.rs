use std::cell::LazyCell;
use std::fmt::format;
use std::iter::Map;
use std::rc::Rc;
use std::time::Instant;

use crate::analysis::analyze;
use crate::errors::macros::internal_error;
use crate::registry::GLOBAL_FUNCS;
use crate::codegen::*;
use crate::parsing::Parser;
use crate::ProgramError;
use crate::types::ValueType;
use crate::typed_values::*;

const VM_STACK_CAPACITY: usize = 16384;
const VM_CALLS_CAPACITY: usize = 256;

// Useful macros
// TODO make these fit with unions also
macro_rules! vm_pop_val {
    ($vm: expr, $variant: ident) => {
        if let TypedValue::$variant(v) = vm_stack_pop!($vm) { v }
        else {
            panic!("{} not on top of stack:\n{:?}", stringify!($variant), $vm.stack)
        }
    };
}
macro_rules! vm_stack_pop {
    ($vm: expr) => {
        $vm.stack.pop().expect("Stack should not be empty.")
    };
}
macro_rules! vm_unwrap_any {
    ($vm: expr, $variant: ident) => {{
        if let TypedValue::$variant(v) = *vm_pop_val!($vm, Any) {
            $vm.stack.push(TypedValue::$variant(v));
        } else {
            return Err($vm.runtime_error(RuntimeError::TypeError));
        }
    }};
}
macro_rules! vm_binary_op {
    ($vm: expr, $variant: ident, $op: tt, $to: ident) => {{
        let rhs = vm_pop_val!($vm, $variant);
        let lhs = vm_pop_val!($vm, $variant);
        $vm.stack.push(TypedValue::$to(lhs $op rhs))
    }};
}

pub fn interpret(src: &str) -> Result<(), ProgramError> {
    let parser = Parser::from(src);
    let stmts = parser.parse()
        .map_err(|errs| ProgramError::ParseError(errs))?;
    let context = analyze(&stmts)
        .map_err(|errs| ProgramError::UsageError(errs))?;
    let main_script = generate(&stmts, context);
    drop(stmts);
    #[cfg(feature = "debug")]
    println!("==================== EXECUTION START ====================");
    run(main_script)
}

pub fn from(src: &str) -> Result<VM, ProgramError> {
    let parser = Parser::from(src);
    let stmts = parser.parse()
        .map_err(|errs| ProgramError::ParseError(errs))?;
    let context = analyze(&stmts)
        .map_err(|errs| ProgramError::UsageError(errs))?;
    let main_script = generate(&stmts, context);
    drop(stmts);
    Ok(VM::new(main_script))
}

pub fn run(function: TypedFunction) -> Result<(), ProgramError> {
    if cfg!(feature = "dont_run") {
        Ok(())
    } else {   
        VM::new(function).run()
        .map(|_| ())
    }
}

pub struct VM {
    // TODO move ip & ref to top frame to VM directly
    call_stack: Vec<Frame>,
    stack: Vec<TypedValue>,
    globals: Vec<TypedValue>,
    /// During tests, this represents the instant of creation, not execution start.
    // runtime_start: Instant,
    // TODO cached summons
    // modules: Map<std::path::Path, Value>

    #[cfg(feature = "runtime_trace")]
    prev_line: u32,
}

struct Frame {
    function: Rc<TypedFunction>,
    ip: usize,
    stack_bottom: usize,
}

impl Frame {
    fn new(function: TypedFunction, stack_bottom: usize) -> Self {
        Self { function: Rc::new(function), ip: 0, stack_bottom }
    }
}

#[derive(Debug, PartialEq)]
pub enum RuntimeError {
    TypeError,
    InvalidCode,
    StackEmpty,
    JumpFail,
    ManualCrash,
}

#[derive(PartialEq)]
enum SuccessStatus {
    Continue, // Continue to the next instruction.
    End,      // Return from the execution loop.

    #[cfg(test)]
    ReturnTop(TypedValue), // Pause execution and return the top of the stack
}

impl VM {
    pub fn new(function: TypedFunction) -> Self {
        let mut new = Self {
            call_stack: Vec::with_capacity(64),
            stack: Vec::with_capacity(VM_STACK_CAPACITY),
            globals: Vec::with_capacity(VM_CALLS_CAPACITY),
            #[cfg(feature = "runtime_trace")]
            prev_line: 0
        };
        new.call_stack.push(Frame::new(function, 0));
        for (_, val) in GLOBAL_FUNCS.iter() {
            new.globals.push(val.clone());
        }
        new
    }

    /// Runs until the status is `End` or `ReturnTop`.
    #[cfg(test)]
    pub fn run_with_input(&mut self, input: TypedValue) -> Result<TypedValue, ProgramError> {
        loop {
            match self.cycle(&input)? {
                SuccessStatus::Continue => {},
                SuccessStatus::End => { return Ok(TypedValue::None); },
                SuccessStatus::ReturnTop(value) => { return Ok(value); },
            }
        }
    }

    #[cfg(test)]
    pub fn run(&mut self) -> Result<TypedValue, ProgramError> {
        self.run_with_input(TypedValue::None)
    }

    #[cfg(not(test))]
    pub fn run(&mut self) -> Result<(), ProgramError> {
        while self.cycle(&TypedValue::None)? == SuccessStatus::Continue {}
        Ok(())
    }

    #[inline(always)]
    // this function will never be called outside of a loop so i just want to
    fn cycle(&mut self, input: &TypedValue) -> Result<SuccessStatus, ProgramError> {

        // Redefine macros according to self
        macro_rules! pop_val {
            ($variant: ident) => { vm_pop_val!(self, $variant) }
        }
        macro_rules! binary_op {
            ($variant: ident $op: tt: $to: ident) => { vm_binary_op!(self, $variant, $op, $to) }
        }
        macro_rules! unwrap_any {
            ($variant: ident) => { vm_unwrap_any!(self, $variant) }
        }
        macro_rules! stack_pop {
            () => { vm_stack_pop!(self) };
        }

        if self.stack.len() >= VM_STACK_CAPACITY {
            panic!("VM value stack overflow!");
        }

        let byte = self.read_instruction();
        #[cfg(feature = "runtime_trace")]
        {
            let ip = self.top_frame().ip - 1;
            let line = self.top_frame().function.get_line(ip);
            let linestr: String = if line == self.prev_line {
                "   :".into()
            } else {
                format!("{}",line)
            };
            println!("{linestr:>4} | {ip:04} Op::{:?}",
                byte.as_ref().unwrap_or(&OpCode::Pass));
            self.prev_line = line;
        }
        let Some(instr) = byte else {
            return Err(self.runtime_error(RuntimeError::InvalidCode));
        };

        match instr {
            OpCode::Pass => {}
            OpCode::None => self.stack.push(TypedValue::None),
            OpCode::True => self.stack.push(TypedValue::Bool(true)),
            OpCode::False => self.stack.push(TypedValue::Bool(false)),
            OpCode::LoadByte => {
                let int = self.read_byte().cast_signed() as i64;
                self.stack.push(TypedValue::Int(int));
            }
            OpCode::Jump => {
                let offset =  self.read_short() as i16;
                self.top_frame().ip = self.top_frame().ip.strict_add_signed(offset as isize);
            }
            OpCode::JumpIfNot => {
                let b = pop_val!(Bool);
                let offset =  self.read_short() as i16;
                if !b {
                    self.top_frame().ip = self.top_frame().ip.strict_add_signed(offset as isize);
                }
            }
            OpCode::CallUser => {
                // stack top atp: [func,arg1...argN]
                let num_args = self.read_byte() as usize;
                let new_bottom = self.stack.len() - num_args - 1;
                let TypedValue::Function(function) = self.stack[new_bottom].clone() else {
                    internal_error!("Expected a function, got {:?}", self.stack[new_bottom])
                };
                #[cfg(feature = "runtime_trace")]
                {
                    println!("\t-->--> Entering {}", function.name);
                }
                self.call_stack.push(Frame { function, ip: 0, stack_bottom: new_bottom });
            }
            OpCode::CallNative => {
                let num_args = self.read_byte() as usize;
                let new_bottom = self.stack.len() - num_args - 1;
                let args = &self.stack[new_bottom + 1..];
                let TypedValue::NativeFunc(func) = self.stack[new_bottom].clone() else {
                    internal_error!("Expected a function, got {:?}", self.stack[new_bottom])
                };
                let result = (func.func)(args);
                self.stack.truncate(new_bottom);
                self.stack.push(result);
            }
            OpCode::GetLocal => {
                let index = self.top_frame().stack_bottom + self.read_byte() as usize;
                self.stack.push(self.stack[index].clone());
            }
            OpCode::SetLocal => {
                let index = self.top_frame().stack_bottom + self.read_byte() as usize;
                self.stack[index] = self.stack_peek(0);
            }
            OpCode::GetGlobal => {
                let index = self.read_byte() as usize;
                self.stack.push(self.globals[index].clone());
            }
            OpCode::SetGlobal => {
                let index = self.read_byte() as usize;
                self.globals[index] = self.stack_peek(0);
            }
            OpCode::DefineGlobal => {
                let new = stack_pop!();
                self.globals.push(new);
            }
            OpCode::Constant => {
                let index = self.read_byte() as usize;
                let new = self.top_frame().function.constants[index].clone();
                self.stack.push(new);
            }
            OpCode::Pop => { self.stack.pop(); },
            OpCode::Return => {
                #[cfg(feature = "runtime_trace")]
                {
                    println!("\t<--<-- Exiting {}", self.top_frame().function.name);
                }
                if self.call_stack.len() == 1 {
                    return Ok(SuccessStatus::End);
                } else {
                    let prev_frame = self.call_stack.pop().unwrap();
                    let result = stack_pop!();
                    self.stack.truncate(prev_frame.stack_bottom);
                    self.stack.push(result);
                }
            }
            OpCode::Crash => return Err(self.runtime_error(RuntimeError::ManualCrash)),
            #[cfg(test)]
            OpCode::TESTTakeInput => {
                self.stack.push(input.clone());
            }
            #[cfg(test)]
            OpCode::TESTYield => {
                let value = stack_pop!();
                self.stack.push(TypedValue::None);
                return Ok(SuccessStatus::ReturnTop(value));
            }
            OpCode::AnyToInt => unwrap_any!(Int),
            OpCode::AnyToFloat => unwrap_any!(Float),
            OpCode::AnyToBool => unwrap_any!(Bool),
            OpCode::AnyToChar => unwrap_any!(Char),
            OpCode::AnyToString => unwrap_any!(String),
            OpCode::IntToFloat => {
                let f = pop_val!(Int) as f64;
                self.stack.push(TypedValue::Float(f));
            }
            OpCode::IntToBool => todo!(),
            OpCode::CharToInt => todo!(),
            OpCode::BoolToInt => todo!(),
            OpCode::BoolToFloat => todo!(),
            OpCode::WrapAny => {
                let val = stack_pop!();
                self.stack.push(TypedValue::Any(Box::new(val)));
            }
            OpCode::ValEqual => {
                let b = stack_pop!();
                let a = stack_pop!();
                self.stack.push(TypedValue::Bool(a == b));
            },
            OpCode::FloatAdd => binary_op!(Float +: Float),
            OpCode::FloatSub => binary_op!(Float -: Float),
            OpCode::FloatMul => binary_op!(Float *: Float),
            OpCode::FloatDiv => binary_op!(Float /: Float),
            OpCode::FloatMod => binary_op!(Float %: Float),
            OpCode::FloatLess => binary_op!(Float <: Bool),
            OpCode::FloatGreater => binary_op!(Float >: Bool),
            OpCode::IntAdd => binary_op!(Int +: Int),
            OpCode::IntSub => binary_op!(Int -: Int),
            OpCode::IntMul => binary_op!(Int *: Int),
            OpCode::IntDiv => binary_op!(Int /: Int),
            OpCode::IntMod => binary_op!(Int %: Int),
            OpCode::IntAnd => binary_op!(Int &: Int),
            OpCode::IntXor => binary_op!(Int ^: Int),
            OpCode::IntOr  => binary_op!(Int |: Int),
            OpCode::IntShl => binary_op!(Int <<: Int),
            OpCode::IntShr => binary_op!(Int >>: Int),
            OpCode::IntLess => binary_op!(Int <: Bool),
            OpCode::IntGreater => binary_op!(Int >: Bool),
            OpCode::FloatNegate => {
                let f = pop_val!(Float);
                self.stack.push(TypedValue::Float(-f));
            }
            OpCode::IntNegate => {
                let i = pop_val!(Int);
                self.stack.push(TypedValue::Int(-i));
            }
            OpCode::IntNot => {
                let i = pop_val!(Int);
                self.stack.push(TypedValue::Int(!i));
            }
            OpCode::BoolNot => {
                let b = pop_val!(Bool);
                self.stack.push(TypedValue::Bool(!b));
            }
        }
        #[cfg(feature = "runtime_trace")]
        {
            let bottom = self.top_frame().stack_bottom;
            println!("   : |\tS=[{}]<-", &self.stack[bottom..].iter()
                .map(|v| format!("{}", v))
                .collect::<Vec<String>>()
                .join("] [")
            );
            // println!("\tG={:?}", self.globals);
        }
        Ok(SuccessStatus::Continue)
    }

    fn top_frame(&mut self) -> &mut Frame {
        self.call_stack.last_mut().unwrap()
    }

    /// Clones the value on the stack at `dist` from the end.
    fn stack_peek(&self, dist: usize) -> TypedValue {
        self.stack[self.stack.len() - 1 - dist].clone()
    }

    fn runtime_error(&mut self, reason: RuntimeError) -> ProgramError {
        eprintln!("Runtime error: {:?}", reason);
        ProgramError::RuntimeError(reason)
    }

    fn read_instruction(&mut self) -> Option<OpCode> {
        OpCode::try_from(self.read_byte()).ok()
    }

    fn read_byte(&mut self) -> u8 {
        self.top_frame().ip += 1;
        let ip = self.top_frame().ip - 1;
        self.top_frame().function.code[ip]
    }

    fn read_short(&mut self) -> u16 {
        let mut s = (self.read_byte() as u16) << 8;
        s |= (self.read_byte() as u16);
        s
    }
}