use std::fmt::format;
use std::iter::Map;
use std::rc::Rc;
use std::time::Instant;

use crate::analysis::analyze;
use crate::builtins::GLOBAL_FUNCS;
use crate::codegen::*;
use crate::parsing::Parser;
use crate::ProgramError;
use crate::types::ValueType;
use crate::values::*;

const VM_STACK_CAPACITY: usize = 16384;
const VM_CALLS_CAPACITY: usize = 256;

macro_rules!  vm_pop_val {
    ($vm: expr, $variant: ident) => {
        if let Value::$variant(v) = $vm.stack_pop() { v }
        else {
            panic!("{} not on top of stack:\n{:?}", stringify!($variant), $vm.stack)
        }
    };
}

macro_rules!  vm_unwrap_any {
    ($vm: expr, $variant: ident) => {{
        if let Value::$variant(v) = *vm_pop_val!($vm, Any) {
            $vm.stack.push(Value::$variant(v));
        } else {
            return Err($vm.runtime_error(RuntimeError::TypeError));
        }
    }};
}

macro_rules! vm_binary_op {
    ($vm: expr, $variant: ident, $op: tt, $to: ident) => {{
        let rhs = vm_pop_val!($vm, $variant);
        let lhs = vm_pop_val!($vm, $variant);
        $vm.stack.push(Value::$to(lhs $op rhs))
    }};
}

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
    if cfg!(feature = "dont_run") {
        Ok(())
    } else {   
        VM::new(function).run()
        .map(|_| ())
    }
}

pub struct VM {
    call_stack: Vec<Frame>,
    stack: Vec<Value>,
    globals: Vec<Value>,
    /// During tests, this represents the instant of creation, not execution start.
    runtime_start: Instant,
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
            stack: Vec::with_capacity(VM_STACK_CAPACITY),
            globals: Vec::with_capacity(VM_CALLS_CAPACITY),
            runtime_start: Instant::now(),
            #[cfg(feature = "runtime_trace")]
            prev_line: 0
        };
        new.call_stack.push(Frame::new(function, 0));
        for (_, obj) in GLOBAL_FUNCS {
            new.globals.push(Value::from(obj));
        }
        new
    }

    /// Runs until the status is `End` or `ReturnTop`.
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
        self.runtime_start = Instant::now();
        while self.cycle(&Value::None)? == SuccessStatus::Continue {}
        Ok(())
    }

    fn cycle(&mut self, input: &Value) -> Result<SuccessStatus, ProgramError> {

        macro_rules! pop_val {
            ($variant: ident) => { vm_pop_val!(self, $variant) }
        }

        macro_rules! binary_op {
            ($variant: ident $op: tt: $to: ident) => { vm_binary_op!(self, $variant, $op, $to) }
        }

        macro_rules! unwrap_any {
            ($variant: ident) => { vm_unwrap_any!(self, $variant) }
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
            OpCode::None => self.stack.push(Value::None),
            OpCode::True => self.stack.push(Value::Bool(true)),
            OpCode::False => self.stack.push(Value::Bool(false)),
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
            OpCode::List => {
                let new_list_start = self.stack.len() - self.read_byte() as usize;
                let list = self.stack.split_off(new_list_start);
                self.stack.push(Value::from(list))
            }
            OpCode::IndexGet => {
                let index = pop_val!(Int);
                let Value::List(list) = self.stack_pop() else { panic!() };
                let at_index = list.borrow()[index as usize].clone();
                self.stack.push(at_index);
            }
            OpCode::IndexSet => {
                // as usual, no popping bc set is an expression
                let index = pop_val!(Int);
                let Value::List(list) = self.stack_pop() else { panic!() };
                let new_value = self.stack_peek(2);
                list.borrow_mut()[index as usize] = new_value;
            }
            OpCode::Slice => todo!(),
            OpCode::StrIndex => todo!(),
            OpCode::StrSlice => todo!(),
            OpCode::AnyToInt => unwrap_any!(Int),
            OpCode::AnyToFloat => todo!(),
            OpCode::AnyToBool => todo!(),
            OpCode::AnyToChar => todo!(),
            OpCode::AnyToString => todo!(),
            OpCode::IntToFloat => {
                let f = pop_val!(Int) as f64;
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
            OpCode::ValEqual => {
                let b = self.stack_pop();
                let a = self.stack_pop();
                self.stack.push(Value::Bool(a == b));
            },
            OpCode::Concat => {
                let Value::String(s2) = self.stack_pop() else { panic!() };
                let Value::String(s1) = self.stack_pop() else { panic!() };
                self.stack.push(Value::from((*s1).clone() + s2.as_ref()));
            },
            OpCode::FloatAdd => binary_op!(Float +: Float),
            OpCode::FloatSub => binary_op!(Float -: Float),
            OpCode::FloatMul => binary_op!(Float *: Float),
            OpCode::FloatDiv => binary_op!(Float /: Float),
            OpCode::FloatMod => binary_op!(Float %: Float),
            OpCode::FloatNegate => {
                let f = pop_val!(Float);
                self.stack.push(Value::Float(-f));
            }
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
            OpCode::IntNegate => {
                let i = pop_val!(Int);
                self.stack.push(Value::Int(-i));
            }
            OpCode::IntNot => {
                let i = pop_val!(Int);
                self.stack.push(Value::Int(!i));
            }
            OpCode::IntLess => binary_op!(Int <: Bool),
            OpCode::IntGreater => binary_op!(Int >: Bool),
            OpCode::BoolNot => {
                let b = pop_val!(Bool);
                self.stack.push(Value::Bool(!b));
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
        self.stack[self.stack.len() - dist - 1].clone()
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