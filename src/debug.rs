use crate::expr_ast::*;
use crate::values::*;
use crate::stmt_ast::*;

pub mod opcodes {
    use crate::{registry::GLOBAL_FUNCS, codegen::OpCode, values::{TypedFunction, TypedValue}};

    pub fn disassemble(func: &TypedFunction) {
        // like 'pow(float, int): float'
        let sig = format!("{}({}): {:?}",
            func.name,
            func.params.iter()
                .map(|t| format!("{:?}", t))
                .collect::<Vec<String>>()
                .join(", "),
            func.ret_type);
        println!("======== {} ========", sig);
        // println!("{:^4} {:^4} {:^16} {}", "Ln", "IP", "Code", "Args");

        let mut reader = Reader { func, ip: 0, prev_line: 0 };
        while reader.ip < reader.func.code.len() {
            /**/
            let line = reader.func.get_line(reader.ip);
            let linestr = if line != reader.prev_line {
                format!("{:04}", line)
            } else {
                "   |".into()
            };
            reader.prev_line = line;
            let op = OpCode::try_from(reader.byte()).expect("Should be at the start of an instruction");
            let opstr = format!("{op:?}");
            print!("{linestr} {:04} {opstr:<16} ", reader.ip - 1);
            match op {
                OpCode::Pass
                | OpCode::None
                | OpCode::True
                | OpCode::False
                | OpCode::Pop
                | OpCode::Return
                | OpCode::Crash

                | OpCode::IntToFloat
                | OpCode::BoolToFloat
                | OpCode::BoolToInt
                | OpCode::CharToInt
                | OpCode::IntToBool
                | OpCode::WrapAny
                | OpCode::AnyToInt
                | OpCode::AnyToFloat
                | OpCode::AnyToChar
                | OpCode::AnyToBool
                | OpCode::AnyToString

                | OpCode::ValEqual
                | OpCode::FloatAdd
                | OpCode::FloatSub
                | OpCode::FloatMul
                | OpCode::FloatDiv
                | OpCode::FloatMod
                | OpCode::FloatNegate
                | OpCode::FloatLess
                | OpCode::FloatGreater
                | OpCode::IntAdd
                | OpCode::IntSub
                | OpCode::IntMul
                | OpCode::IntDiv
                | OpCode::IntMod
                | OpCode::IntAnd
                | OpCode::IntXor
                | OpCode::IntOr
                | OpCode::IntShl
                | OpCode::IntShr
                | OpCode::IntNegate
                | OpCode::IntNot
                | OpCode::IntLess
                | OpCode::IntGreater
                | OpCode::BoolNot
                    => println!(),
                #[cfg(test)]
                OpCode::TESTTakeInput
                | OpCode::TESTYield
                    => println!(),
                OpCode::Constant => {
                    println!("{:?}", reader.constant());
                }
                OpCode::GetLocal
                | OpCode::SetLocal
                | OpCode::CallNative
                | OpCode::CallUser
                | OpCode::LoadByte 
                | OpCode::FieldGet 
                | OpCode::FieldSet => {
                    let b= reader.byte();
                    println!("{b:02}");
                }
                OpCode::GetGlobal
                | OpCode::SetGlobal => {
                    let b= reader.byte() as usize;
                    let num_globals = GLOBAL_FUNCS.len();
                    if b < num_globals {
                        println!("{}", GLOBAL_FUNCS[b].0);
                    } else {
                        println!("{:02}", b - num_globals);
                    }
                }
                OpCode::Jump
                | OpCode::JumpIfNot => {
                    // cast to i16 recovers sign of encoded signed short
                    let offset = reader.short() as i16;
                    // cast to isize to satisfy rust
                    let new_index = reader.ip.strict_add_signed(offset as isize);
                    println!("to {new_index}");
                }
            }
        }
        
        println!("======== {} END ========", func.name);
    }
    
    struct Reader<'f> {
        func: &'f TypedFunction,
        ip: usize,
        prev_line: u32,
    }

    impl Reader<'_> {
        fn constant(&mut self) -> &TypedValue {
            let index = self.byte() as usize;
            self.func.constants.get(index).unwrap()
        }

        fn short(&mut self) -> u16 {
            let s = (self.byte() as u16) << 8;
            s | (self.byte() as u16)
        }

        fn byte(&mut self) -> u8 {
            self.ip += 1;
            self.func.code[self.ip - 1]
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum DebugRuntimeError {
    TypeError,
    OperatorError, // should never happen
    StateRequired,
}

struct AstPrinter {
    as_tree: bool,
    depth: i8,
}

pub fn expr_to_str(expr: &Expr, as_tree: bool) -> String {
    String::new()
}

struct DebugAstPrinter {
    depth: usize
}

pub fn stmt_to_str(stmt: &Stmt) -> String {
    DebugAstPrinter {
        depth: 0,
    }.fmt(stmt)
}

impl DebugAstPrinter {
    pub fn fmt(&mut self, stmt: &Stmt) -> String {
        // format!("{}{}", TAB.repeat(self.depth), stmt.accept(self))
        todo!()
    }
}

struct StaticRunner;

pub fn evaluate_static(expr: &Expr) -> Result<TypedValue, DebugRuntimeError> {
    // expr.accept(&mut StaticRunner)
    todo!()
}

fn coerce_types(val1: TypedValue, val2: TypedValue) -> Result<(TypedValue, TypedValue), DebugRuntimeError> {
    use TypedValue as V;
    
    // Implicit casting cases:
    // string & any -> both string
    // int & float -> both float
    // int & char -> both int

    if val1.get_type() == val2.get_type() {
        // Already the same type
        Ok((val1, val2))
    } else if let V::String(_) = val1 {
        // val1 is a string
        Ok((val1, V::from(val2.to_string())))
    } else if let V::String(_) = val2 {
        // val2 is a string
        Ok((V::from(val1.to_string()), val2))
    } else {
        match (&val1, &val2) {
            (V::Int(i1), V::Float(_)) => Ok((V::Float(*i1 as f64), val2)),
            (V::Int(_), V::Char(c2)) => Ok((val1, V::Int(*c2 as i64))),
            (V::Float(_), V::Int(i2)) => Ok((val1, V::Float(*i2 as f64))),
            (V::Char(c1), V::Int(_)) => Ok((V::Int(*c1 as i64), val2)),
            _ => Err(DebugRuntimeError::TypeError),
        }
    }
}

pub fn run_static(stmts: &Vec<Stmt>) -> Result<(), DebugRuntimeError> {
    for stmt in stmts {
        run_static_single(stmt)?;
    }
    Ok(())
}

pub fn run_static_single(stmt: &Stmt) -> Result<(), DebugRuntimeError> {
    // stmt.accept(&mut StaticRunner)
    todo!()
}
