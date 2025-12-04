"""
Get      = obj: Expr | property: Token
Slice    = sequence: Expr | query: Expr
Logical  = left: Expr  | op: Token | right: Expr
"""
exprs = """
Conditional = condition: Expr | if_true: Expr | if_false: Expr
Binary      = left: Expr  | op: Token | right: Expr
Assign      = assignee: Expr | value: Expr
Cast        = expr: Expr | new_type: ValueType
Unary       = op: Token | target: Expr | prefix: bool
Call        = callee: Expr | args: Vec<Expr>
Variable    = identifier: Token
Literal     = repr: Token | val: Value
""".strip()
"""
"""
stmts = """
Function    = ret_type: ValueType | name: Token | params: Vec<(Token, ValueType)> | body: Vec<Stmt> | id: usize
Summon      = path: Vec<Token> | alias: Option<Token> | id: usize
Var         = name: Token | val: Option<Box<Expr>>
Block       = statements: Vec<Stmt>
Expression  = expression: Box<Expr>
If          = condition: Box<Expr> | true_branch: Stmt | false_branch: Option<Stmt>
While       = condition: Box<Expr> | body: Stmt
For         = var: Token | sequence: Box<Expr> | body: Stmt
Keyword     = keyword: Token | arg: Option<Box<Expr>>
""".strip()

copied = "usize".split(",")

def generate(name: str, desc: str, add_id = False):
    desc = [l.split("=") for l in desc.split("\n")]
    desc = [(l[0].strip(), l[1].split("|")) for l in desc]
    desc = [(l[0], [[p.strip() for p in f.split(":")] for f in l[1]]) for l in desc]
    desc = [(l[0], [f if name not in f[1] or f[1] == f"Vec<{name}>" else [f[0], f[1].replace(name, f"Box<{name}>")] for f in l[1]]) for l in desc]
    desc = [(l[0], [f if f[1] not in [n[0] for n in desc] else [f[0], f"{name}::{f[1]}"] for f in l[1]]) for l in desc]
    if add_id:
        desc = [(l[0], l[1] + [["id", "usize"]]) for l in desc]
    # print("\n".join(map(str, desc)))
    # return
    print("#[derive(Debug)]")
    print(f"pub enum {name} {{")
    print( "    // TODO the commented-out ones")
    for node in desc:
        print(f"    {node[0]}", end=" { ")
        for i, field in enumerate(node[1]):
            print(f"{field[0]}: {field[1]}", end="")
            if i < len(node[1]) - 1:
                print(end=", ")
        print(" },")
    print("}")
    print()

    print(f"impl<'me, 'vis> {name} where 'me: 'vis {{")
    print(f"    pub fn dummy() -> Self {{")
    print(f"        Self::")
    print(f"    }}")
    print(f"    ")
    if add_id:
        print(f"    pub fn id(&self) -> usize {{")
        print(f"        match self {{")
        for node in desc:
            print(f"{' '*12}Self::{node[0]} {{ id, .. }} => *id,")
        print(f"        }}")
        print(f"    }}")
    print(f"    pub fn accept<T>(&'me self, visitor: &mut impl {name}Visitor<'vis, T>) -> T {{")
    print( "        match self {")
    for node in desc:
        # no oneline/multiline variant, its short enough probably :)
        print(" "*12+f"Self::{node[0]} {{ ", end="")
        for i, field in enumerate(node[1]):
            print(f"{field[0]}", end="")
            if i < len(node[1]) - 1:
                print(end=", ")
        print(f" }} =>\n{" "*16}visitor.visit_{node[0].lower()}_{name.lower()}(",end="")
        for i, field in enumerate(node[1]):
            print(f"{"*" if field[1] in copied else ""}{field[0]}", end="")
            if i < len(node[1]) - 1:
                print(end=", ")
        print("),")
    print("        }")
    print("    }")
    print("}")
    print()

    visname = f"{name}Visitor"
    print(f"pub trait {visname}<'ast, T> {{")
    for node in desc:
        print(f"    fn visit_{node[0].lower()}_{name.lower()}(&mut self,\n", end=" "*8)
        for i, field in enumerate(node[1]):
            print(f"{field[0]}: {"" if field[1] in copied else "&'ast "}{field[1]}", end="")
            if i < len(node[1]) - 1:
                print(end=", ")
        print(f") -> T;")
    print("}")

import sys

# tree_to_generate = "stmt"
tree_to_generate = sys.argv[1].lower()

print("use crate::lexing::Token;")
print("use crate::values::*;")
if tree_to_generate == "stmt":
    print("use crate::expr_ast::Expr;")
    print()
    generate("Stmt", stmts, add_id=False)
elif tree_to_generate == "expr":
    print()
    generate("Expr", exprs, add_id=True)
else:
    print("\nnothing to generate")