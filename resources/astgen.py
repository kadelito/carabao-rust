"""
Assign      = assignee: Expr | op: Token | value: Expr // op for +=, *=, etc
"""
exprs = """
Object      = fields: Vec<(Token, Expr)>
Conditional = condition: Expr | if_true: Expr | if_false: Expr
Boolean     = left: Expr  | op: Token | right: Expr
Binary      = left: Expr  | op: Token | right: Expr
Assign      = assignee: Expr | value: Expr
Cast        = expr: Expr | new_type: ValueType
Unary       = op: Token | target: Expr | prefix: bool
Slice       = sequence: Expr | query: Expr
Call        = callee: Expr | args: Vec<Expr>
Get         = obj: Expr | property: Token
List        = items: Vec<Expr>
Variable    = identifier: Token
Literal     = repr: Token | val: TypedValue
""".strip()
"""
Try         = try_block: Stmt | exception: Identifier | alias: Identifier | catch_block: Stmt
Switch      = value: Expr | branches: Vec<(Expr, Stmt)>
"""
# TODO switch statements are hard.. start with matching `any`?
stmts = """
Class       = name: Token | 
Function    = ret_type: ValueType | name: Token | params: Vec<(Token, ValueType)> | body: Vec<Stmt>
Summon      = path: Vec<Token> | alias: Option<Token> | id: usize
Var         = name: Token | var_type: Option<ValueType> | val: Option<Box<Expr>>
Block       = statements: Vec<Stmt>
Expression  = expression: Box<Expr>
If          = condition: Box<Expr> | true_branch: Stmt | false_branch: Option<Stmt>
While       = condition: Box<Expr> | body: Stmt
For         = var: Token | sequence: Box<Expr> | body: Stmt
Keyword     = keyword: Token | arg: Option<Box<Expr>>
""".strip()

str_if = lambda s, b: s if b else ""
copied = "usize".split(",")

def generate(name: str, desc: str, add_id = False):
    # remove comments
    desc = [l[:l.index("//") if "//" in l else len(l)] for l in desc.split("\n")]
    # split into [variant, fields]
    desc = [l.split("=") for l in desc]
    # split fields
    desc = [(l[0].strip(), l[1].split("|")) for l in desc]
    # split each field into name and type
    desc = [(l[0], [[p.strip() for p in f.split(":")] for f in l[1]]) for l in desc]
    # add Box<> if a field has the enum's type
    desc = [(l[0], [f if name not in f[1] or f[1].startswith("Vec") else [f[0], f[1].replace(name, f"Box<{name}>")] for f in l[1]]) for l in desc]
    if add_id:
        desc = [(l[0], l[1] + [["id", "usize"]]) for l in desc]

    # =================== ENUM ===================
    # define the enum's variants
    print("#[derive(Debug)]")
    print(f"pub enum {name} {{")
    print( "    // TO""DO the commented-out ones")
    for node in desc:
        print(f"    {node[0]}", end=" { ")
        for i, field in enumerate(node[1]):
            print(f"{field[0]}: {field[1]}", end="")
            if i < len(node[1]) - 1:
                print(end=", ")
        print(" },")
    print("}")
    print()

    # =================== IMPL ===================
    # define .dummy() and .accept() for each of the enum's variants via `match`
    # if the enum has an `id` field, return it from each variant
    print(f"impl<'me, 'vis> {name} where 'me: 'vis {{")
    print(f"    pub fn dummy() -> Self {{")
    print(f"        Self::todo!()")
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
        print(" "*12+f"Self::{node[0]} {{ ", end="")
        for i, field in enumerate(node[1]):
            print(f"{field[0]}", end="")
            if i < len(node[1]) - 1:
                print(end=", ")
        print(f" }} =>\n{" "*16}visitor.visit_{node[0].lower()}_{name.lower()}(",end="")
        for i, field in enumerate(node[1]):
            print(f"{str_if("*", field[1] in copied)}{field[0]}", end="")
            if i < len(node[1]) - 1:
                print(end=", ")
        print("),")
    print("        }")
    print("    }")
    print("}")
    print()

    # =================== VISITOR ===================
    # define a visitor for the enum & its methods
    visname = f"{name}Visitor"
    print(f"pub trait {visname}<'ast, T> {{")
    for node in desc:
        print(f"    fn visit_{node[0].lower()}_{name.lower()}(&mut self,\n", end=" "*8)
        for i, field in enumerate(node[1]):
            print(f"{field[0]}: {str_if("&'ast ", field[1] not in copied)}{field[1]}", end="")
            if i < len(node[1]) - 1:
                print(end=", ")
        print(f") -> T;")
    print("}")

import sys

tree_to_generate = sys.argv[1].lower()

print("use crate::lexing::Token;")
print("use crate::types::*;")
if tree_to_generate == "stmt":
    print("use crate::expr_ast::Expr;")
    print()
    generate("Stmt", stmts, add_id=False)
elif tree_to_generate == "expr":
    print("use crate::values::*;")
    print()
    generate("Expr", exprs, add_id=True)
else:
    print("\nnothing to generate")