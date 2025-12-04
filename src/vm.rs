use crate::analysis::analyze;
use crate::codegen::OpCode;
use crate::codegen::generate;
use crate::parsing::Parser;
use crate::ProgramError;
use crate::values::Function;
use crate::values::Object;
use crate::values::Value;

pub fn interpret(src: &str) -> Result<(), ProgramError> {

    #[cfg(feature = "debug")] {
        println!("Input:\n\"\"\"\n{}\n\"\"\"", src);
        // println!("Input:\n\"\"\"\n{}\n\"\"\"", src.replace("\r\n", "\\n\r\n")); // crlf i hate you
    }
    let parser = Parser::from(src);
    let stmts = parser.parse()
        .map_err(|e| ProgramError::ParseError(e))?;
    let context = analyze(&stmts)
        .ok_or(ProgramError::UsageError)?;
    let function = generate(&stmts, context);
    drop(stmts);
    #[cfg(feature = "debug")]
    println!("==================== EXECUTION START ====================");
    run(function)
    // run_static(&stmts).map_err(|e| ProgramError::DebugError(e))?;
}

// TODO profile & decide where to newline

pub fn run(function: Function) -> Result<(), ProgramError> {
    VM::new(function).run()
}

struct VM {
    // TODO call stack
    ip: usize,
    stack: Vec<Value>,
    function: Function,
    globals: Vec<Value>,
}

struct Frame {
    stack_bottom: usize,
    ip: usize,
}

#[derive(Debug)]
pub enum RuntimeError {
    TypeError,
    InvalidCode,
    StackEmpty,
    JumpFail,
}

impl VM {
    fn new(function: Function) -> Self {
        Self {
            ip: 0,
            stack: Vec::with_capacity(1024),
            globals: Vec::with_capacity(64),
            function,
        }
    }

    fn run(&mut self) -> Result<(), ProgramError> {
        while self.ip < self.function.code.len() {
            let byte = self.read_instruction();
            #[cfg(feature = "debug")]
            println!("{:04} Op::{:?}", self.ip-1, byte.as_ref().unwrap_or(&OpCode::Pass));
            let Some(instr) = byte else {
                return Err(self.runtime_error(RuntimeError::InvalidCode));
            };
            match instr {
                OpCode::Pass => {},
                OpCode::None => self.stack.push(Value::None),
                OpCode::True => self.stack.push(Value::Bool(true)),
                OpCode::False => self.stack.push(Value::Bool(false)),
                OpCode::Jump => {
                    let offset =  self.read_short() as i16;
                    self.ip = self.ip.strict_add_signed(offset as isize);
                },
                OpCode::JumpIfNot => {
                    let Value::Bool(b) = self.stack_pop() else { panic!() };
                    let offset =  self.read_short() as i16;
                    if !b {
                        self.ip = self.ip.strict_add_signed(offset as isize);
                    }
                },
                OpCode::Call => {
                    // stack top: [func,arg1...argN]
                    let num_args = self.read_byte() as usize;
                    let new_bottom = self.stack.len() - num_args - 1;
                    todo!();
                }
                OpCode::GetLocal => {
                    let index = self.read_byte() as usize;
                    self.stack.push(self.stack[index].clone());
                },
                OpCode::SetLocal => {
                    let index = self.read_byte() as usize;
                    self.stack[index] = self.stack_peek(0);
                }
                OpCode::GetGlobal => {
                    let index = self.read_byte() as usize;
                    self.stack.push(self.globals[index].clone());
                },
                OpCode::SetGlobal => {
                    let index = self.read_byte() as usize;
                    self.globals[index] = self.stack_peek(0);
                },
                OpCode::DefineGlobal => {
                    let new = self.stack_pop();
                    self.globals.push(new);
                },
                OpCode::Constant => {
                    let index = self.read_byte() as usize;
                    let new = self.function.constants[index].clone();
                    self.stack.push(new);
                },
                OpCode::Pop => {
                    self.stack.pop();
                },
                OpCode::Return => {
                    // TODO actual returning
                    println!("{:?}", self.stack_pop())
                }
                OpCode::ValEqual => self.binary_val(|a, b| Value::Bool(a == b)),
                OpCode::Concat => {
                    let Value::Object(obj1) = self.stack_pop() else { panic!() };
                    let Object::String(s1) = (*obj1).clone() else { panic!() };
                    let Value::Object(obj2) = self.stack_pop() else { panic!() };
                    let Object::String(s2) = obj2.as_ref() else { panic!() };
                    self.stack.push(Value::from(s1 + s2));
                }
                OpCode::FloatAdd => self.binary_float(|a, b| Value::Float(a + b)),
                OpCode::FloatSub => self.binary_float(|a, b| Value::Float(a - b)),
                OpCode::FloatMul => self.binary_float(|a, b| Value::Float(a * b)),
                OpCode::FloatDiv => self.binary_float(|a, b| Value::Float(a / b)),
                OpCode::FloatMod => self.binary_float(|a, b| Value::Float(a % b)),
                OpCode::FloatNegate => {
                    let Value::Float(f) = self.stack_pop() else { panic!() };
                    self.stack.push(Value::Float(-f));
                },
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
                    let Value::Int(i) = self.stack_pop() else { panic!() };
                    self.stack.push(Value::Int(-i));
                },
                OpCode::IntNot => {
                    let Value::Int(i) = self.stack_pop() else { panic!() };
                    self.stack.push(Value::Int(!i));
                },
                OpCode::IntLess => self.binary_int(|a, b| Value::Bool(a < b)),
                OpCode::IntGreater => self.binary_int(|a, b| Value::Bool(a > b)),
                OpCode::BoolAnd => {
                    let Value::Bool(b) = self.stack_pop() else { panic!() };
                    let Value::Bool(a) = self.stack_pop() else { panic!() };
                    self.stack.push(Value::Bool(a && b));
                }
                OpCode::BoolOr => {
                    let Value::Bool(b) = self.stack_pop() else { panic!() };
                    let Value::Bool(a) = self.stack_pop() else { panic!() };
                    self.stack.push(Value::Bool(a || b));
                }
                OpCode::BoolNot => {
                    let Value::Bool(b) = self.stack_pop() else { panic!() };
                    self.stack.push(Value::Bool(!b));
                },
                            }
            #[cfg(feature = "debug")] {
                println!("\tS={:?}<-", self.stack);
                println!("\tG={:?}", self.globals);
            }
        }
        Ok(())
    }

    fn stack_pop(&mut self) -> Value {
        self.stack.pop().expect("Stack should not be empty.")
    }

    fn stack_peek(&self, dist: usize) -> Value {
        self.stack.get(self.stack.len() - dist - 1).unwrap().clone()
    }

    fn binary_float(&mut self, func: fn(f64, f64) -> Value) {
        let Value::Float(b) = self.stack_pop() else { panic!() };
        let Value::Float(a) = self.stack_pop() else { panic!() };
        self.stack.push(func(a, b));
    }

    fn binary_int(&mut self, func: fn(i64, i64) -> Value) {
        let Value::Int(b) = self.stack_pop() else { panic!() };
        let Value::Int(a) = self.stack_pop() else { panic!() };
        self.stack.push(func(a, b));
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
        self.ip += 1;
        self.function.code[self.ip - 1]
    }

    fn read_short(&mut self) -> u16 {
        let mut s = (self.read_byte() as u16) << 8;
        s |= (self.read_byte() as u16);
        s
    }
}