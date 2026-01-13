import re
"""
Object      = fields: Vec<(Token, Expr)>
"""
exprs = """
Conditional = condition: Expr | if_true: Expr | if_false: Expr
Boolean     = left: Expr  | op: Token | right: Expr
Binary      = left: Expr  | op: Token | right: Expr
Assign      = assignee: Expr | op: Token | value: Expr // op for +=, *=, etc
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
Struct      = name: Token | fields: Vec<(Token, ValueType)>
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

str_if        = lambda s, b: s if b else ""
copied = {"usize"}
optioninner = lambda s: s[7:-1]
isoption = lambda s: s.startswith("Option")
boxinner = lambda s: s[4:-1]
isbox = lambda s: s.startswith("Box")

borrowed = lambda typ, name: (
    f"{name}: Option<&'ast {optioninner(typ)[4:-1]}>" 
        if isoption(typ) and isbox(optioninner(typ))
    else f"{name}: Option<&'ast {optioninner(typ)}>"
        if isoption(typ)
    else f"{name}: {str_if("&'ast ", typ not in copied)}{boxinner(typ) if isbox(typ) else typ}"
)
borrowvar = lambda typ, name: (
    f"*{name}"
        if typ in copied
    else f"{name}.as_deref()"
        if isoption(typ) and isbox(optioninner(typ))
    else f"{name}.as_ref()"
        if isoption(typ) or isbox(typ)
    else name
)
mutborrowed = lambda typ, name: (
    f"{name}: Option<&'ast mut {optioninner(typ)[4:-1]}>" 
        if isoption(typ) and isbox(optioninner(typ))
    else f"{name}: Option<&'ast mut {optioninner(typ)}>"
        if isoption(typ)
    else f"{name}: {str_if("&'ast mut ", typ not in copied)}{boxinner(typ) if isbox(typ) else typ}"
)
mutborrowvar = lambda typ, name: (
    f"*{name}"
        if typ in copied
    else f"{name}.as_deref_mut()"
        if isoption(typ) and isbox(optioninner(typ))
    else f"{name}.as_mut()"
        if isoption(typ) or isbox(typ)
    else name
)

def generate(name: str, desc: str, add_id = False, invader = False):
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
    print( "#[derive(Debug)]")
    print(f"pub enum {name} {{")
    for node in desc:
        print(f"    {node[0]}", end=" { ")
        for i, field in enumerate(node[1]):
            print(f"{field[0]}: {field[1]}", end="")
            if i < len(node[1]) - 1:
                print(end=", ")
        print( " },")
    print( "}")
    print()
    
    # =================== IMPL DEFAULT ===================
    print(f"impl Default for {name} {{")
    print(f"    /// Returns a dummy value when a{str_if('n'), name[0].lower() in "aeiou"} {name} is expected but some error occured.")
    print(f"    /// It is expected that this {name} never actually gets examined.")
    print( "    fn default() -> Self {")
    print( "        ")
    print( "    }")
    print( "}")
    print()

    # =================== IMPL ===================
    # define .accept() for each of the enum's variants via `match`
    # if the enum has an `id` field, return it from each variant
    print(f"impl<'me, 'vis> {name} where 'me: 'vis {{")
    if add_id:
        print(f"    pub fn id(&self) -> usize {{")
        print(f"        match self {{")
        for node in desc:
            print(f"{' '*12}Self::{node[0]} {{ id, .. }} => *id,")
        print(f"        }}")
        print(f"    }}")
        print(f"    ")
    print(f"    pub fn accept<T>(&'me self, visitor: &mut impl {name}Visitor<'vis, T>) -> T {{")
    print( "        match self {")
    for node in desc:
        print( " "*12+f"Self::{node[0]} {{ ", end="")
        for i, field in enumerate(node[1]):
            print(f"{field[0]}", end="")
            if i < len(node[1]) - 1:
                print(end=", ")
        print(f" }} =>\n{" "*16}visitor.visit_{node[0].lower()}_{name.lower()}(",end="")
        for i, field in enumerate(node[1]):
            print(f"{borrowvar(field[1], field[0])}", end="")
            if i < len(node[1]) - 1:
                print(end=", ")
        print( "),")
    print( "        }")
    print( "    }")
    if invader:
        print( "    ")
        print(f"    pub fn yield_to<T>(&'me mut self, invader: &mut impl {name}Invader<'vis, T>) -> T {{")
        print( "        match self {")
        for node in desc:
            print( " "*12+f"Self::{node[0]} {{ ", end="")
            for i, field in enumerate(node[1]):
                print(f"{field[0]}", end="")
                if i < len(node[1]) - 1:
                    print(end=", ")
            print(f" }} =>\n{" "*16}invader.invade_{node[0].lower()}_{name.lower()}(",end="")
            for i, field in enumerate(node[1]):
                print(f"{mutborrowvar(field[1], field[0])}", end="")
                if i < len(node[1]) - 1:
                    print(end=", ")
            print( "),")
        print( "        }")
        print( "    }")
    print( "}")
    print()

    # =================== VISITOR ===================
    # define a visitor for the enum & its methods
    visname = f"{name}Visitor"
    print(f"pub trait {visname}<'ast, T> {{")
    for node in desc:
        print(f"    fn visit_{node[0].lower()}_{name.lower()}(&mut self,\n", end=" "*8)
        for i, field in enumerate(node[1]):
            print(f"{borrowed(field[1], field[0])}", end="")
            if i < len(node[1]) - 1:
                print(end=", ")
        print(f") -> T;")
    print( "}")
    print()

    # =================== INVADER (MUTATING VISITOR) ===================
    # define a visitor for the enum & its methods
    visname = f"{name}Invader"
    print(f"/// A mutating {name} visitor")
    print(f"pub trait {visname}<'ast, T> {{")
    for node in desc:
        print(f"    fn invade_{node[0].lower()}_{name.lower()}(&mut self,\n", end=" "*8)
        for i, field in enumerate(node[1]):
            print(f"{mutborrowed(field[1], field[0])}", end="")
            if i < len(node[1]) - 1:
                print(end=", ")
        print(f") -> T;")
    print( "}")

import sys

tree_to_generate = sys.argv[1].lower()

print( "use crate::lexing::Token;")
print( "use crate::types::*;")
if tree_to_generate == "stmt":
    print("use crate::expr_ast::Expr;")
    print()
    generate("Stmt", stmts, add_id=False, invader=False)
elif tree_to_generate == "expr":
    print("use crate::values::*;")
    print()
    generate("Expr", exprs, add_id=True, invader=False)
else:
    print( "\nnothing to generate")