use std::{collections::{HashMap, HashSet}, ops::IndexMut};

use crate::{expr_ast::*, lexing::*, stmt_ast::Stmt, types::*, values::*};

/**
 * A parser for Carabao.
 */
pub struct Parser<'a> {
    source: Lexer<'a>,
    /// The `token` to be consumed next.
    cur: Token,
    /// The `token` most recently consumed.
    ///
    /// `prev` is an `Option` to let the AST take ownership without cloning.
    prev: Option<Token>,
    ignore_newlines: bool,
    next_id: usize,
    errors: Vec<ParseError>,
    panic_mode: bool,
    strings: HashMap<String, TypedValue>,
    /// yes its global
    /// no i dont care
    user_types: HashSet<String>,
}

const VALID_TYPES: [TokenType; 6] = {
    use TokenType::*;
    [Any, Int, Bool, Float, Char, String]
};

impl<'a> From<Lexer<'a>> for Parser<'a> {
    fn from(token_source: Lexer<'a>) -> Self {
        Parser {
            source: token_source,
            cur: Token::dummy(),
            prev: None,
            ignore_newlines: false,
            next_id: 0,
            errors: Vec::new(),
            panic_mode: false,
            strings: HashMap::new(),
            user_types: HashSet::new(),
        }
    }
}

/*
* ======================================================
* ======================================================
*                  Statement parsing
*/

impl<'a> Parser<'a> {
    pub fn parse(mut self) -> Result<Vec<Stmt>, Vec<ParseError>> {
        self.advance(); // initializes self.cur

        let mut stmts = Vec::new();
        while !self.at_end() {
            stmts.push(self.declaration());
            self.skip_newlines();
        }
        if !self.errors.is_empty() {
            Err(self.errors)
        } else {
            Ok(stmts)
        }
    }

    fn declaration(&mut self) -> Stmt {
        self.skip_newlines();
        let decl = if self.try_consume(TokenType::Func) {
            self.function_def()
        } else if self.try_consume(TokenType::New) {
            self.var_def()
        } else if self.try_consume(TokenType::Summon) {
            self.import()
        } else if self.try_consume(TokenType::Struct) {
            self.struct_def()
        } else {
            self.statement()
        };

        if self.panic_mode {
            self.synchronize();
        }

        decl
    }

    fn struct_def(&mut self) -> Stmt {
        let name = self.expect_binding();
        self.expect(TokenType::OpenBrace);
        let mut fields = Vec::new();
        while !self.at_end() {
            let field_type = self.expect_type();
            let field = self.expect_binding();
            fields.push((field, field_type));
            // TODO change this if we need some delimiter
            self.skip_newlines();
            if self.try_consume(TokenType::CloseBrace) {
                break;
            }
        }
        self.user_types.insert(name.copy_ident());
        Stmt::Struct { name, fields }
    }

    fn function_def(&mut self) -> Stmt {
        let name = self.expect_binding();
        let mut params = Vec::new();
        self.expect(TokenType::OpenParen);
        if !self.check(TokenType::CloseParen) {
            loop {
                let val_type = self.expect_type();
                let param = self.expect_binding();
                params.push((param, val_type));
                // TODO variable length paramaters & default arguments

                if !self.try_consume(TokenType::Comma) {
                    break;
                }
            }
        }
        self.expect(TokenType::CloseParen);
        let ret_type = if self.try_consume(TokenType::Colon) {
            self.expect_type()
        } else {
            ValueType::None
        };
        self.expect(TokenType::OpenBrace);
        let Stmt::Block { statements } = self.block() else {
            return Stmt::dummy();
        };
        Stmt::Function {
            ret_type,
            name,
            params,
            body: statements,
        }
    }

    fn var_def(&mut self) -> Stmt {
        let var_type = self.try_consume_type();
        let name = self.expect_binding();
        let val = if self.try_consume(TokenType::Equal) {
            Some(Box::new(self.expression(false)))
        } else {
            None
        };
        Stmt::Var {
            name,
            var_type,
            val,
        }
    }

    fn import(&mut self) -> Stmt {
        let mut path = Vec::new();
        self.expect(TokenType::Identifier);
        path.push(self.take_prev());
        while self.try_consume(TokenType::Dot) {
            self.expect(TokenType::Identifier);
            path.push(self.take_prev());
        }

        let mut alias = None;
        if self.try_consume(TokenType::As) {
            alias = Some(self.expect_binding());
        }

        Stmt::Summon {
            path,
            alias,
            id: self.new_id(),
        }
    }

    fn statement(&mut self) -> Stmt {
        self.skip_newlines();
        if self.try_consume(TokenType::OpenBrace) {
            self.block()
        } else if self.try_consume(TokenType::If) {
            self.if_stmt()
        } else if self.try_consume(TokenType::While) {
            self.while_loop()
        } else if self.try_consume(TokenType::For) {
            self.for_loop()
        } else if self.try_consume_any(&vec![
            TokenType::Return,
            TokenType::Break,
            TokenType::Continue,
        ]) {
            self.keyword_stmt()
        } else {
            let stmt = self.expression_stmt();
            stmt
        }
    }

    fn block(&mut self) -> Stmt {
        let mut statements = Vec::new();
        while !self.at_end() && !self.check(TokenType::CloseBrace) {
            statements.push(self.declaration());
            self.skip_newlines(); // so in `{...stmt() \n }`, the '}' is seen 
        }
        self.expect_because(TokenType::CloseBrace, ParseError::BraceNotClosed);
        Stmt::Block { statements }
    }

    fn if_stmt(&mut self) -> Stmt {
        let condition = Box::new(self.expression(false));
        let true_branch = if self.try_consume_any(&[TokenType::Newline, TokenType::Colon]) {
            self.statement()
        } else if self.try_consume(TokenType::OpenBrace) {
            self.block()
        } else {
            self.error_at_next(ParseError::NoStatement);
            Stmt::dummy()
        };
        let true_branch = Box::new(true_branch);

        self.skip_newlines();
        let false_branch = if self.try_consume(TokenType::Else) {
            Some(Box::new(self.statement()))
        } else {
            None
        };
        Stmt::If {
            condition,
            true_branch,
            false_branch,
        }
    }

    fn while_loop(&mut self) -> Stmt {
        let condition = Box::new(self.expression(false));
        let body = if self.try_consume_any(&[TokenType::Newline, TokenType::Colon]) {
            self.statement()
        } else if self.try_consume(TokenType::OpenBrace) {
            self.block()
        } else {
            self.error_at_next(ParseError::NoStatement);
            Stmt::dummy()
        };
        let body = Box::new(body);
        Stmt::While { condition, body }
    }

    fn for_loop(&mut self) -> Stmt {
        let var = self.expect_binding();
        self.expect(TokenType::In);
        let sequence = Box::new(self.expression(false));
        let body = if self.try_consume_any(&[TokenType::Newline, TokenType::Colon]) {
            self.statement()
        } else if self.try_consume(TokenType::OpenBrace) {
            self.block()
        } else {
            self.error_at_next(ParseError::NoStatement);
            Stmt::dummy()
        };
        let body = Box::new(body);

        Stmt::For {
            var,
            sequence,
            body,
        }
    }

    fn keyword_stmt(&mut self) -> Stmt {
        let keyword = self.take_prev();
        let arg = if !self.at_stmt_end() {
            Some(Box::new(self.expression(false)))
        } else {
            None
        };
        self.expect_stmt_end();
        Stmt::Keyword { keyword, arg }
    }

    fn expression_stmt(&mut self) -> Stmt {
        let expression = Box::new(self.expression(false));
        self.expect_stmt_end();
        Stmt::Expression { expression }
    }
}

/*
* ======================================================
* ======================================================
*                  Expression parsing
*/

impl<'a> Parser<'a> {
    pub fn parse_expr_string(source: &str) -> Result<Expr, ParseError> {
        let lexer: Lexer<'_> = Lexer::new(&source);
        let mut parser = Parser::from(lexer);
        parser.advance();
        let result = parser.expression(false);
        if !parser.errors.is_empty() {
            Err(parser.errors[0])
        } else {
            // println!("{}", to_str(&result, false));
            Ok(result)
        }
    }

    fn expression(&mut self, ignore_newlines: bool) -> Expr {
        self.skip_newlines();
        let old = self.ignore_newlines;
        self.ignore_newlines = ignore_newlines;
        let expr = self.assign();
        self.ignore_newlines = old;
        expr
    }

    fn assign(&mut self) -> Expr {
        let mut expr = self.ternary();
        if self.try_consume_any(&[TokenType::Equal]) {
            let op = self.take_prev();
            let assignee = Box::new(expr);
            let value = Box::new(self.expression(false));
            let value = Box::new(self.expression(false));
            expr = Expr::Assign {
                assignee,
                op,
                value,
                id: self.new_id(),
            };
        }
        expr
    }

    fn ternary(&mut self) -> Expr {
        let mut expr = self.logic_or();
        if self.try_consume(TokenType::Question) {
            let left = Box::new(expr);
            let middle = Box::new(self.expression(true));
            self.expect_because(TokenType::Colon, ParseError::IncompleteTernary);
            let right = Box::new(self.expression(false));
            expr = Expr::Conditional {
                condition: left,
                if_true: middle,
                if_false: right,
                id: self.new_id(),
            };
        }
        expr
    }

    fn logic_or(&mut self) -> Expr {
        self.left_assoc_boolean_series(Parser::logic_and, &[TokenType::DoubleVertBar])
    }

    fn logic_and(&mut self) -> Expr {
        self.left_assoc_boolean_series(Parser::equality, &[TokenType::DoubleAmpersand])
        // self.left_assoc_boolean_series(Parser::keyword_bin_op, &[TokenType::DoubleAmpersand])
    }

    /// Helper for logic_or & logic_and.
    /// Instead of returning a single operand or an Expr::Binary,
    /// returns a single operand or an Expr::Boolean.
    fn left_assoc_boolean_series(
        &mut self,
        operand: fn(&mut Self) -> Expr,
        operators: &[TokenType],
    ) -> Expr {
        // case when no left operand,
        // continue ahead if it's a unary prefix
        if !self.check_any(&[
            TokenType::Bang,
            TokenType::Minus,
            TokenType::Tilde,
            #[cfg(test)]
            TokenType::DoubleLess,
            #[cfg(test)]
            TokenType::DoubleGreater,
        ]) && self.try_consume_any(operators)
        {
            self.error_at_prev(ParseError::BinOpNoLeft);
            return self.left_assoc_bin_series(operand, operators);
        }

        let mut left = operand(self);
        if self.try_consume_any(operators) {
            let op = self.take_prev();
            self.skip_newlines();
            let right = self.left_assoc_boolean_series(operand, operators);
            left = Expr::Boolean {
                left: Box::new(left),
                op,
                right: Box::new(right),
                id: self.new_id(),
            };
        }
        left
    }

    fn keyword_bin_op(&mut self) -> Expr {
        self.left_assoc_bin_series(Parser::equality, &[TokenType::In])
    }

    fn equality(&mut self) -> Expr {
        self.left_assoc_bin_series(
            Parser::comparison,
            &[TokenType::DoubleEqual, TokenType::BangEqual],
        )
    }

    fn comparison(&mut self) -> Expr {
        self.left_assoc_bin_series(
            Parser::range,
            Parser::range,
            &[
                TokenType::Less,
                TokenType::LessEqual,
                TokenType::Greater,
                TokenType::GreaterEqual,
            ],
        )
    }

    fn range(&mut self) -> Expr {
        self.left_assoc_bin_series(Parser::bitwise_or, &[TokenType::DoubleDot])
    }

    fn bitwise_or(&mut self) -> Expr {
        self.left_assoc_bin_series(Parser::bitwise_excl_or, &[TokenType::VertBar])
    }

    fn bitwise_excl_or(&mut self) -> Expr {
        self.left_assoc_bin_series(Parser::bitwise_and, &[TokenType::Carrot])
    }

    fn bitwise_and(&mut self) -> Expr {
        self.left_assoc_bin_series(Parser::bit_shift, &[TokenType::Ampersand])
    }

    fn bit_shift(&mut self) -> Expr {
        self.left_assoc_bin_series(
            Parser::term,
            &[TokenType::DoubleLess, TokenType::DoubleGreater],
        )
    }

    fn term(&mut self) -> Expr {
        self.left_assoc_bin_series(Parser::factor, &[TokenType::Plus, TokenType::Minus])
    }

    fn factor(&mut self) -> Expr {
        self.left_assoc_bin_series(
            Parser::cast,
            &[TokenType::Star, TokenType::FSlash, TokenType::Percent],
        )
    }

    fn cast(&mut self) -> Expr {
        let expr = self.unary();
        if self.try_consume(TokenType::As) {
            let expr = Box::new(expr);
            let new_type = self.expect_type();
            Expr::Cast {
                expr,
                new_type,
                id: self.new_id(),
            }
        } else {
            expr
        }
    }

    /// Parses a binary expression with `operand`
    /// and any of `operators`, equal in precedence
    fn left_assoc_bin_series(
        &mut self,
        operand: fn(&mut Self) -> Expr,
        operators: &[TokenType],
    ) -> Expr {
        // case when no left operand,
        // continue ahead if it's a unary prefix
        if !self.check_any(&[
            TokenType::Bang,
            TokenType::Minus,
            TokenType::Tilde,
            #[cfg(test)]
            TokenType::DoubleLess,
            #[cfg(test)]
            TokenType::DoubleGreater,
        ]) && self.try_consume_any(operators)
        {
            self.error_at_prev(ParseError::BinOpNoLeft);
            return self.left_assoc_bin_series(operand, operators);
        }

        let mut left = operand(self);
        while self.try_consume_any(operators) {
            let op = self.take_prev();
            self.skip_newlines();
            let right = operand(self);
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
                id: self.new_id(),
            };
        }
        left
    }

    fn unary(&mut self) -> Expr {
        if self.try_consume_any(&[
            TokenType::Bang,
            TokenType::Minus,
            TokenType::Tilde,
            #[cfg(test)]
            TokenType::DoubleLess,
            #[cfg(test)]
            TokenType::DoubleGreater,
        ]) {
            self.skip_newlines();
            let op = self.take_prev();
            // recursive bc right-associative
            let target = self.unary();
            return Expr::Unary {
                op,
                target: Box::new(target),
                prefix: true,
                id: self.new_id(),
            };
        }
        self.call_or_similar()
    }

    /// `calls()`, `.gets`, `.methods()` and `indexing[]`.
    ///
    /// All are suffixes and obv the same precedence, so they go together
    fn call_or_similar(&mut self) -> Expr {
        // TODO named arguments
        let mut obj = self.primary();
        loop {
            let id = self.new_id();
            if self.try_consume(TokenType::OpenParen) {
                let callee = Box::new(obj);
                let args = self.expr_list(TokenType::CloseParen);
                obj = Expr::Call { callee, args, id };
            } else if self.try_consume(TokenType::Dot) {
                self.expect(TokenType::Identifier);
                let attribute = self.take_prev();
                obj = Expr::Get {
                    obj: Box::new(obj),
                    property: attribute,
                    id,
                };
            } else if self.try_consume(TokenType::OpenBracket) {
                let query = Box::new(self.expression(true));
                self.expect(TokenType::CloseBracket);
                obj = Expr::Slice {
                    sequence: Box::new(obj),
                    query,
                    id,
                }
            } else {
                break;
            }
        }
        obj
    }

    fn expr_list(&mut self, end_token: TokenType) -> Vec<Expr> {
        let mut list = Vec::new();
        if !self.check(end_token) {
            // no do-while :(
            list.push(self.expression(true));
            while self.try_consume(TokenType::Comma) {
                list.push(self.expression(true));
            }
        }
        self.expect(end_token);
        list
    }

    fn primary(&mut self) -> Expr {
        let id = self.new_id();
        if self.try_consume(TokenType::True) {
            return Expr::Literal {
                repr: self.take_prev(),
                val: TypedValue::Bool(true),
                id,
            };
        } else if self.try_consume(TokenType::False) {
            return Expr::Literal {
                repr: self.take_prev(),
                val: TypedValue::Bool(false),
                id,
            };
        } else if self.try_consume(TokenType::None) {
            return Expr::Literal {
                repr: self.take_prev(),
                val: TypedValue::None,
                id,
            };
        } else if self.try_consume_any(&[
            TokenType::DecIntLiteral,
            TokenType::HexIntLiteral,
            TokenType::BinIntLiteral,
        ]) {
            return self.parse_integer(id);
        } else if self.try_consume(TokenType::FloatLiteral) {
            let mut literal = self.take_prev();
            let value: Result<f64, ParseError> = literal
                .take_lexeme()
                .unwrap()
                .parse()
                .map_err(|_e| ParseError::ParseFloatError);
            match value {
                Ok(f) => {
                    return Expr::Literal {
                        repr: literal,
                        val: TypedValue::Float(f),
                        id,
                    };
                }
                Err(e) => {
                    self.error_at(&literal, e);
                    return Expr::dummy();
                }
            }
        } else if self.try_consume(TokenType::StringLiteral) {
            let literal = self.take_prev();
            let mut raw = literal.lexeme().clone();
            raw = raw[1..raw.len() - 1].to_owned();
            if let Some(string) = self.strings.get(&raw) {
                return Expr::Literal {
                    repr: literal,
                    val: string.clone(),
                    id,
                };
            }
            let mut raw_chars = raw.chars();
            let mut val = String::new();
            while let Some(c) = raw_chars.next() {
                if c == '\\' {
                    let char = if let Some(escaped) = raw_chars.next() {
                        match escaped {
                            '\\' => '\\',
                            'n' => '\n',
                            'r' => '\r',
                            't' => '\t',
                            '"' => '\"',
                            _ => {
                                self.error_at(&literal, ParseError::InvalidEscapeCharacter);
                                '\\'
                            }
                        }
                    } else {
                        // This will (probably) never happen,
                        // but just in case '\' is the last character in the string
                        // without another '\' preceding.
                        '\\'
                    };
                    val.push(char);
                } else {
                    val.push(c);
                }
            }
            val.shrink_to_fit();
            let val = TypedValue::from(val);

            // it wasn't there before, so we insert it here with the formatted string
            // Note that the pointer
            self.strings.insert(raw, val.clone());

            return Expr::Literal {
                repr: literal,
                val,
                id,
            };
        } else if self.try_consume(TokenType::CharLiteral) {
            let mut literal = self.take_prev();
            let lexeme = literal.take_lexeme().unwrap();
            let lexeme = lexeme.chars().collect::<Box<[char]>>();
            let val = if lexeme[1] == '\\' {
                let c = match lexeme[2] {
                    '\\' => '\\',
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '"' => '\"',
                    _ => {
                        self.error_at(&literal, ParseError::InvalidEscapeCharacter);
                        '\\'
                    }
                };
                TypedValue::Char(c)
            } else {
                TypedValue::Char(lexeme[1])
            };
            return Expr::Literal {
                repr: literal,
                val,
                id,
            };
        } else if self.try_consume(TokenType::OpenParen) {
            let expr = self.expression(true);
            self.expect_because(TokenType::CloseParen, ParseError::ParenNotClosed);
            return expr;
        } else if self.try_consume(TokenType::OpenBracket) {
            let items = self.expr_list(TokenType::CloseBracket);
            return Expr::List { items, id };
        } else if self.try_consume(TokenType::Identifier) {
            let identifier = self.take_prev();
            return Expr::Variable { identifier, id };
        } else if self.try_consume(TokenType::Func) {
            todo!("Anonymous functions/lambdas")
        }
        self.error_at_next(ParseError::NoExpression);
        Expr::dummy()
    }
}

/*
* ======================================================
* ======================================================
*                   Helper Methods
*/

impl<'a> Parser<'a> {
    fn synchronize(&mut self) {
        self.panic_mode = false;

        while !self.at_end() {
            // newline doesnt count, we could be inside an expression
            if let Some(prev) = &self.prev
                && prev.kind() == TokenType::Semicolon
            {
                self.advance();
                return;
            }
            match self.peek().kind() {
                // TODO other statement starts?
                TokenType::New
                | TokenType::Struct
                | TokenType::Summon
                | TokenType::Func
                | TokenType::If
                | TokenType::For
                | TokenType::While
                | TokenType::Return
                | TokenType::Break
                | TokenType::Continue => {
                    break;
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn parse_integer(&mut self, id: usize) -> Expr {
        let mut literal = self.take_prev();
        let mut str = literal.take_lexeme().unwrap().into_boxed_str();
        let base = match literal.kind() {
            TokenType::DecIntLiteral => 10,
            TokenType::HexIntLiteral => 16,
            TokenType::BinIntLiteral => 12,
            _ => unreachable!(),
        };
        if base != 10 {
            // truncate the 0x or 0b
            if str.len() == 2 {
                // just "0x" or "0b", becomes 0
                str = str.index_mut(1..).into();
            } else {
                str = str.index_mut(2..).into();
            }
        }
        let value = i64::from_str_radix(&str, base).map_err(|_| ParseError::ParseIntError);
        match value {
            Ok(i) => Expr::Literal {
                repr: literal,
                val: TypedValue::Int(i),
                id,
            },
            Err(e) => {
                self.error_at(&literal, e);
                Expr::dummy()
            }
        }
    }

    fn new_id(&mut self) -> usize {
        self.next_id += 1;
        self.next_id
    }

    fn expect_binding(&mut self) -> Token {
        self.expect_because(TokenType::Identifier, ParseError::NotAssignTarget);
        self.take_prev()
    }

    fn try_consume_type(&mut self) -> Option<ValueType> {
        if self.try_consume_any(&VALID_TYPES) {
            let token = self.take_prev();
            let mut intermediate = ValueType::from_token(token).unwrap();
            while self.try_consume(TokenType::OpenBracket) {
                // Consume brackets until not
                // for int[][][]... or something
                self.expect(TokenType::CloseBracket);
                intermediate = ValueType::List(Box::new(intermediate));
            }
            Some(intermediate)
        } else if self.check(TokenType::Identifier) && self.user_types.contains(self.peek().lexeme()) {
            self.advance();
            let token = self.take_prev();
            let mut intermediate = ValueType::from_token(token).unwrap();
            while self.try_consume(TokenType::OpenBracket) {
                // Consume brackets until not
                // for int[][][]... or something
                self.expect(TokenType::CloseBracket);
                intermediate = ValueType::List(Box::new(intermediate));
            }
            Some(intermediate)    
        } else if self.try_consume(TokenType::Func) {
            self.expect(TokenType::OpenParen);
            let mut params = Vec::new();
            if !self.check(TokenType::CloseParen) {
                params.push(self.expect_type());
                while self.try_consume(TokenType::Comma) {
                    params.push(self.expect_type());
                }
            };
            self.expect(TokenType::CloseParen);
            let ret_type = if self.try_consume(TokenType::Colon) {
                self.expect_type()
            } else {
                ValueType::None
            };

            let params = params.into_boxed_slice();
            Some(ValueType::Function(
                FunctionType {
                    ret_type,
                    params,
                    native: false,
                }
                .into(),
            ))
        } else {
            None
        }
    }

    fn expect_type(&mut self) -> ValueType {
        self.skip_newlines();
        let _type = self.try_consume_type();
        if let Some(_type) = _type {
            _type
        } else {
            self.error_at_next(ParseError::NoValueType);
            ValueType::None
        }
    }

    fn expect_stmt_end(&mut self) {
        if !self.try_consume_any(&vec![
            TokenType::Newline,
            TokenType::Semicolon,
            TokenType::EOF,
        ]) {
            self.error_at_next(ParseError::NoStmtEnd);
        }
    }

    fn at_stmt_end(&self) -> bool {
        self.check_any(&vec![
            TokenType::Newline,
            TokenType::Semicolon,
            TokenType::EOF,
        ])
    }

    fn skip_newlines(&mut self) {
        while !self.at_end() && self.try_consume(TokenType::Newline) {}
    }

    fn try_consume(&mut self, kind: TokenType) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn try_consume_any(&mut self, kinds: &[TokenType]) -> bool {
        for kind in kinds {
            if self.try_consume(*kind) {
                return true;
            }
        }
        false
    }

    fn expect(&mut self, kind: TokenType) {
        self.expect_because(kind, ParseError::ExpectedToken(kind));
    }

    /// SKIPS NEWLINES!!!
    fn expect_because(&mut self, kind: TokenType, if_not: ParseError) {
        self.skip_newlines();
        if self.check(kind) {
            self.advance();
        } else {
            self.error_at_next(if_not);
            // dont consume bc idk
        }
    }

    fn check(&self, kind: TokenType) -> bool {
        self.peek().kind() == kind
    }

    fn check_any(&self, kinds: &[TokenType]) -> bool {
        for kind in kinds {
            if self.check(*kind) {
                return true;
            }
        }
        false
    }

    fn at_end(&self) -> bool {
        self.peek().kind() == TokenType::EOF
    }

    fn take_prev(&mut self) -> Token {
        self.prev.take().unwrap_or(Token::dummy())
    }

    // so i dont accidentally overwrite cur or something. more readable too probably
    fn peek(&self) -> &Token {
        &self.cur
    }

    // Advances until pointing at a non-error token.
    fn advance(&mut self) -> &Token {
        // dark magic to allow moving out of self.cur
        // basically self.prev = self.cur.take()
        self.prev = Some(std::mem::replace(&mut self.cur, Token::dummy()));

        loop {
            self.cur = self.source.scan_token();

            if let TokenType::Error = self.cur.kind() {
                self.error_at_next(ParseError::TokenError(self.cur.error().unwrap()));
            } else if self.ignore_newlines && self.check(TokenType::Newline) {
                // continue loop
            } else {
                break;
            }
        }
        self.prev.as_ref().unwrap()
    }

    fn error_at_next(&mut self, reason: ParseError) {
        // even though error_at doesn't modify self.cur,
        // rust doesn't know that :( so we clone
        self.error_at(&self.peek().clone(), reason);
    }

    fn error_at_prev(&mut self, reason: ParseError) {
        // see error_at_next
        self.error_at(
            &self.prev.clone().expect("Error should use an unused token"),
            reason,
        );
    }

    fn error_at(&mut self, token: &Token, reason: ParseError) {
        if self.panic_mode {
            return;
        } // consequent errors are ignored until synchronized
        self.panic_mode = true;

        eprintln!(
            "Error on line {} (at {:?}) : {:?}",
            token.line(),
            token,
            reason
        );

        self.errors.push(reason.clone());
    }
}

impl<'a> From<&'a str> for Parser<'a> {
    fn from(value: &'a str) -> Self {
        Self::from(Lexer::new(value))
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ParseError {
    TokenError(TokenizationError),
    NoExpression,
    BinOpNoLeft, // like '* 2'
    ParseIntError,
    ParseFloatError,
    ParenNotClosed,
    BraceNotClosed,
    IncompleteTernary,
    NoStmtEnd,
    ExprIsNotStmt,
    NoStatement,
    NotAssignTarget,
    NoDeclaration,
    NoValueType,
    InvalidEscapeCharacter,

    ExpectedToken(TokenType),

    Unspecified,
}

#[cfg(test)]
mod parsing_tests {
    use crate::debug::evaluate_static;

    #[allow(unstable_features)]
    use crate::{
        lexing::{lexing_tests::make_token, *},
        parsing::*,
    };

    #[test]
    fn helpers() {
        let mut tester = Parser::from(Lexer::new("1 2 3"));
        assert_eq!(tester.cur, Token::dummy());
        assert_eq!(tester.prev, None);
        tester.advance(); // advances to 1
        //  [1][2][3]
        //...^
        assert_eq!(tester.cur, make_token("1", 1));
        assert_eq!(tester.prev, Some(Token::dummy()));
        assert!(tester.check(TokenType::DecIntLiteral));
        assert!(tester.try_consume(TokenType::DecIntLiteral)); // advances to 2
        assert!(tester.check(TokenType::DecIntLiteral));
        assert!(tester.try_consume(TokenType::DecIntLiteral)); // advances to 2
        //  [1][2][3]
        //  (1) ^
        assert_eq!(tester.cur, make_token("2", 1));
        assert_eq!(tester.prev, Some(make_token("1", 1)));
        assert_eq!(tester.take_prev(), make_token("1", 1));
        assert_eq!(tester.prev, None);
        tester.expect_because(TokenType::DecIntLiteral, ParseError::NoExpression); // advances to 3
        tester.expect_because(TokenType::DecIntLiteral, ParseError::NoExpression); // advances to 3
        assert_eq!(tester.prev, Some(make_token("2", 1)));
        tester.advance(); // advances past 3
        //  [1] [2] [3] EOF EOF EOF...
        //          (3)  ^
        assert!(tester.at_end());
        assert_eq!(tester.cur.kind(), TokenType::EOF);
        assert_eq!(tester.prev, Some(make_token("3", 1)));
    }

    #[test]
    fn literals() {
        {
            let one = Parser::parse_expr_string("1").unwrap();
            let Expr::Literal { repr: _, val, .. } = one else {
                panic!("'one' did not match the pattern.")
            };
            let TypedValue::Int(i) = val else {
                panic!("'i' did not match the pattern.")
            };
            assert_eq!(i, 1);
        }
        {
            let pi = Parser::parse_expr_string("3.14159").unwrap();
            let Expr::Literal { repr: _, val, .. } = pi else {
                panic!("'pi' did not match the pattern.")
            };
            let TypedValue::Float(f) = val else {
                panic!("'i' did not match the pattern.")
            };
            assert_eq!(f, 3.14159)
        }
    }
}
