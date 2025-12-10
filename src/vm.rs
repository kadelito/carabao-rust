use std::fmt::format;
use std::iter::Map;
use std::rc::Rc;

use crate::analysis::analyze;
use crate::builtins::GLOBAL_FUNCS;
use crate::codegen::*;
use crate::parsing::Parser;
use crate::ProgramError;
use crate::types::ValueType;
use crate::values::*;

pub fn interpret(src: &str) -> Result<(), ProgramError> {

    #[cfg(feature = "debug")] {
        println!("Input:\n\"\"\"\n{}\n\"\"\"", src);
        // println!("Input:\n\"\"\"\n{}\n\"\"\"", src.replace("\r\n", "\\n\r\n")); // crlf i hate you
    }
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

pub fn run(function: Function) -> Result<(), ProgramError> {
    VM::new(function).run()
        .map(|_| ())
}

pub struct VM {
    call_stack: Vec<Frame>,
    stack: Vec<Value>,
    globals: Vec<Value>,
    // TODO cached summons
    // modules: Map<std::path::Path, Value>

    #[cfg(feature = "runtime_trace")]
    prev_line: u32,
}

struct Frame {
    function: Rc<Function>,
    ip: usize,
    stack_bottom: usize,
}

impl Frame {
    fn new(function: Function, stack_bottom: usize) -> Self {
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
    ReturnTop(Value), // Pause execution and return the top of the stack
}

impl VM {
    pub fn new(function: Function) -> Self {
        let mut new = Self {
            call_stack: Vec::with_capacity(64),
            stack: Vec::with_capacity(1024),
            globals: Vec::with_capacity(64),
            #[cfg(feature = "runtime_trace")]
            prev_line: 0
        };
        new.call_stack.push(Frame::new(function, 0));
        for (_, obj) in GLOBAL_FUNCS {
            new.globals.push(Value::from(obj));
        }
        new
    }

    /// Runs until it encounters a
    #[cfg(test)]
    pub fn run_with_input(&mut self, input: Value) -> Result<Value, ProgramError> {
        loop {
            match self.cycle(&input)? {
                SuccessStatus::Continue => {},
                SuccessStatus::End => { return Ok(Value::None); },
                SuccessStatus::ReturnTop(value) => { return Ok(value); },
            }
        }
    }

    #[cfg(test)]
    pub fn run(&mut self) -> Result<Value, ProgramError> {
        self.run_with_input(Value::None)
    }

    #[cfg(not(test))]
    pub fn run(&mut self) -> Result<(), ProgramError> {
        while self.cycle(&Value::None)? == SuccessStatus::Continue {}
        Ok(())
    }

    fn cycle(&mut self, input: &Value) -> Result<SuccessStatus, ProgramError> {
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
            OpCode::None => self.stack.push(Value::None),
            OpCode::True => self.stack.push(Value::Bool(true)),
            OpCode::False => self.stack.push(Value::Bool(false)),
            OpCode::Jump => {
                let offset =  self.read_short() as i16;
                self.top_frame().ip = self.top_frame().ip.strict_add_signed(offset as isize);
            }
            OpCode::JumpIfNot => {
                let b = self.pop_bool();
                let offset =  self.read_short() as i16;
                if !b {
                    self.top_frame().ip = self.top_frame().ip.strict_add_signed(offset as isize);
                }
            }
            OpCode::SwapTop => {
                let end = self.stack.len() - 1;
                let index = end - self.read_byte() as usize;
                self.stack.swap(index, end);
            }
            OpCode::Call => {
                // stack top atp: [func,arg1...argN]
                let num_args = self.read_byte() as usize;
                let new_bottom = self.stack.len() - num_args - 1;
                match self.stack_peek(num_args) {
                    Value::Function(function) => {
                        #[cfg(feature = "runtime_trace")]
                        {
                            println!("\t-->--> Entering {}", function.name);
                        }
                        self.call_stack.push(Frame { function, ip: 0, stack_bottom: new_bottom });
                    }
                    Value::NativeFunc(function) => {
                        let NativeFunction { func, .. } = function.as_ref();
                        let args = &self.stack[new_bottom + 1..];
                        let result = func(args);
                        self.stack.truncate(new_bottom);
                        self.stack.push(result);
                    }
                    _ => panic!()
                }
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
                let new = self.stack_pop();
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
                    let result = self.stack_pop();
                    self.stack.truncate(prev_frame.stack_bottom);
                    self.stack.push(result);
                }
            }
            OpCode::IntToFloat => {
                let f = self.pop_int() as f64;
                self.stack.push(Value::Float(f));
            }
            OpCode::IntToBool => todo!(),
            OpCode::CharToInt => todo!(),
            OpCode::BoolToInt => todo!(),
            OpCode::BoolToFloat => todo!(),
            OpCode::WrapAny => {
                let val = self.stack_pop();
                self.stack.push(Value::Any(Box::new(val)));
            }
            OpCode::AnyToInt => {
                let Value::Int(i) = self.pop_any() else {
                    return Err(self.runtime_error(RuntimeError::TypeError));
                };
                self.stack.push(Value::Int(i));
            }
            OpCode::AnyToFloat => {
                let Value::Float(v) = self.pop_any() else {
                    return Err(self.runtime_error(RuntimeError::TypeError));
                };
                self.stack.push(Value::Float(v));
            }
            OpCode::AnyToBool => {
                let Value::Bool(v) = self.pop_any() else {
                    return Err(self.runtime_error(RuntimeError::TypeError));
                };
                self.stack.push(Value::Bool(v));
            }
            OpCode::AnyToChar => {
                let Value::Char(v) = self.pop_any() else {
                    return Err(self.runtime_error(RuntimeError::TypeError));
                };
                self.stack.push(Value::Char(v));
            }
            OpCode::AnyToString => {
                let Value::String(v) = self.pop_any() else {
                    return Err(self.runtime_error(RuntimeError::TypeError));
                };
                self.stack.push(Value::String(v));
            }
            OpCode::ValEqual => self.binary_val(|a, b| Value::Bool(a == b)),
            OpCode::Concat => {
                let Value::String(s2) = self.stack_pop() else { panic!() };
                let Value::String(s1) = self.stack_pop() else { panic!() };
                self.stack.push(Value::from((*s1).clone() + s2.as_ref()));
            }
            OpCode::FloatAdd => self.binary_float(|a, b| Value::Float(a + b)),
            OpCode::FloatSub => self.binary_float(|a, b| Value::Float(a - b)),
            OpCode::FloatMul => self.binary_float(|a, b| Value::Float(a * b)),
            OpCode::FloatDiv => self.binary_float(|a, b| Value::Float(a / b)),
            OpCode::FloatMod => self.binary_float(|a, b| Value::Float(a % b)),
            OpCode::FloatNegate => {
                let f = self.pop_float();
                self.stack.push(Value::Float(-f));
            }
            OpCode::FloatLess => self.binary_float(|a, b| Value::Bool(a < b)),
            OpCode::FloatGreater => self.binary_float(|a, b| Value::Bool(a > b)),
            OpCode::IntAdd => self.binary_int(|a, b| Value::Int(a + b)),
            OpCode::IntSub => self.binary_int(|a, b| Value::Int(a - b)),
            OpCode::IntMul => self.binary_int(|a, b| Value::Int(a * b)),
            OpCode::IntDiv => self.binary_int(|a, b| Value::Int(a / b)),
            OpCode::IntMod => self.binary_int(|a, b| Value::Int(a % b)),
            OpCode::IntAnd => self.binary_int(|a, b| Value::Int(a & b)),
            OpCode::IntXor => self.binary_int(|a, b| Value::Int(a ^ b)),
            OpCode::IntOr  => self.binary_int(|a, b| Value::Int(a | b)),
            OpCode::IntShl => self.binary_int(|a, b| Value::Int(a << b)),
            OpCode::IntShr => self.binary_int(|a, b| Value::Int(a >> b)),
            OpCode::IntNegate => {
                let i = self.pop_int();
                self.stack.push(Value::Int(-i));
            }
            OpCode::IntNot => {
                let i = self.pop_int();
                self.stack.push(Value::Int(!i));
            }
            OpCode::IntLess => self.binary_int(|a, b| Value::Bool(a < b)),
            OpCode::IntGreater => self.binary_int(|a, b| Value::Bool(a > b)),
            OpCode::BoolNot => {
                let b = self.pop_bool();
                self.stack.push(Value::Bool(!b));
            }
            OpCode::Crash => return Err(self.runtime_error(RuntimeError::ManualCrash)),
            #[cfg(test)]
            OpCode::TESTTakeInput => {
                self.stack.push(input.clone());
            }
            #[cfg(test)]
            OpCode::TESTYield => {
                let value = self.stack_pop();
                self.stack.push(Value::None);
                return Ok(SuccessStatus::ReturnTop(value));
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

    fn stack_pop(&mut self) -> Value {
        self.stack.pop().expect("Stack should not be empty.")
    }

    /// Clones the value on the stack at `dist` from the end.
    fn stack_peek(&self, dist: usize) -> Value {
        self.stack.get(self.stack.len() - dist - 1).unwrap().clone()
    }

    /// Pops an `Any` value from the stack, returning the wrapped value.
    /// Panics if the top value was not of type `Any`.
    fn pop_any(&mut self) -> Value {
        if let Value::Any(v) = self.stack_pop() { *v }
        else { panic!() }
    }

    fn binary_float(&mut self, func: fn(f64, f64) -> Value) {
        let b = self.pop_float();
        let a = self.pop_float();
        self.stack.push(func(a, b));
    }


    /// Pops a `Float` value from the stack, returning the primitive value.
    /// Panics if the top value was not of type `Float`.
    fn pop_float(&mut self) -> f64 {
        if let Value::Float(f) = self.stack_pop() { f }
        else { panic!() }
    }

    /// Pops a `Int` value from the stack, returning the primitive value.
    /// Panics if the top value was not of type `Int`.
    fn binary_int(&mut self, func: fn(i64, i64) -> Value) {
        let b = self.pop_int();
        let a = self.pop_int();
        self.stack.push(func(a, b));
    }

    fn pop_int(&mut self) -> i64 {
        if let Value::Int(i) = self.stack_pop() { i }
        else { panic!() }
    }

    fn pop_bool(&mut self) -> bool {
        if let Value::Bool(b) = self.stack_pop() { b }
        else { panic!() }
    }
    
    fn binary_val(&mut self, func: fn(Value, Value) -> Value) {
        let b = self.stack_pop();
        let a = self.stack_pop();
        self.stack.push(func(a, b));
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