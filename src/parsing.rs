use crate::{
    debug::expr_to_str,
    expr_ast::*,
    lexing::*,
    stmt_ast::Stmt,
    values::{Value, ValueType},
};

pub struct Parser<'a> {
    source: Lexer<'a>,
    /// The `token` to be consumed
    cur: Token,
    /// The `token` most recently consumed.
    ///
    /// If no tokens have been consumed, `prev` is `TokenType::Error`
    /// with `EmptyToken` as the reason.
    prev: Option<Token>,
    ignore_newlines: bool,
    next_id: usize,
    first_error: Option<ParseError>,
    panic_mode: bool,
}

const VALUE_TYPES: [TokenType; 6] = {
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
            first_error: None,
            panic_mode: false,
        }
    }
}

/*
* ======================================================
* ======================================================
*                  Statement parsing
*/

impl<'a> Parser<'a> {
    pub fn parse(mut self) -> Result<Vec<Stmt>, ParseError> {
        self.advance(); // initializes self.cur

        let mut stmts = Vec::new();
        while !self.at_end() {
            stmts.push(self.declaration());
        }
        if let Some(e) = self.first_error {
            Err(e)
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
        // TODO actually implement import
        // } else if self.try_consume(TokenType::Summon) {
        //     self.import()
        } else {
            self.statement()
        };

        if self.panic_mode {
            self.synchronize();
        }

        decl
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
            id: self.new_id(),
        }
    }

    fn var_def(&mut self) -> Stmt {
        let name = self.expect_binding();
        self.expect(TokenType::Equal);
        let val = if !self.at_stmt_end() {
            Some(Box::new(self.expression()))
        } else {
            None
        };
        Stmt::Var { name, val }
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

        Stmt::Summon { path, alias, id: self.new_id() }
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
            self.skip_newlines();
        }
        self.expect_because(TokenType::CloseBrace, ParseError::BraceNotClosed);
        Stmt::Block { statements }
    }

    fn if_stmt(&mut self) -> Stmt {
        let condition = Box::new(self.expression());
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
        let condition = Box::new(self.expression());
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
        let sequence = Box::new(self.expression());
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
            Some(Box::new(self.expression()))
        } else {
            None
        };
        Stmt::Keyword { keyword, arg }
    }

    fn expression_stmt(&mut self) -> Stmt {
        let expression = Box::new(self.expression());
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
        let result = parser.expression();
        if let Some(e) = parser.first_error {
            Err(e)
        } else {
            // println!("{}", to_str(&result, false));
            Ok(result)
        }
    }

    fn expression(&mut self) -> Expr {
        self.skip_newlines();
        let expr = self.assign();
        expr
    }

    fn assign(&mut self) -> Expr {
        let mut expr = self.ternary();
        if self.try_consume_any(&[TokenType::Equal]) {
            let assignee = Box::new(expr);
            let value = Box::new(self.assign());
            expr = Expr::Assign {
                assignee,
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
            let middle = Box::new(self.expression());
            self.expect_because(TokenType::Colon, ParseError::IncompleteTernary);
            let right = Box::new(self.expression());
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
        self.left_assoc_bin_series(Parser::logic_and, &[TokenType::DoubleVertBar])
    }

    fn logic_and(&mut self) -> Expr {
        // self.left_assoc_bin_series(Parser::keyword_bin_op, &[TokenType::DoubleAmpersand])
        self.left_assoc_bin_series(Parser::equality, &[TokenType::DoubleAmpersand])
    }

    fn equality(&mut self) -> Expr {
        self.left_assoc_bin_series(
            Parser::comparison,
            &[TokenType::DoubleEqual, TokenType::BangEqual],
        )
    }

    fn comparison(&mut self) -> Expr {
        self.left_assoc_bin_series(
            Parser::bitwise_or,
            &[
                TokenType::Less,
                TokenType::LessEqual,
                TokenType::Greater,
                TokenType::GreaterEqual,
            ],
        )
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

    fn left_assoc_bin_series(
        &mut self,
        operand: fn(&mut Self) -> Expr,
        operators: &[TokenType],
    ) -> Expr {
        // case when no left operand,
        // continue ahead if it's a unary prefix
        if !self.check_any(&[TokenType::Bang, TokenType::Minus, TokenType::Tilde])
            && self.try_consume_any(operators)
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
        if self.try_consume_any(&[TokenType::Bang, TokenType::Minus, TokenType::Tilde]) {
            self.skip_newlines();
            let op = self.take_prev();
            let target = self.unary();
            return Expr::Unary {
                op,
                target: Box::new(target),
                prefix: true,
                id: self.new_id(),
            };
        }
        self.call()
    }

    /// Calls(), .gets, indexing[]
    fn call(&mut self) -> Expr {
        let callee = self.primary();
        if self.try_consume(TokenType::OpenParen) {
            let callee = Box::new(callee);
            let mut args = Vec::new();
            if !self.check(TokenType::CloseParen) {
                // no do-while :(
                args.push(self.expression());
                while self.try_consume(TokenType::Comma) {
                    args.push(self.expression());
                }
            }
            self.expect(TokenType::CloseParen);
            Expr::Call {
                callee,
                args,
                id: self.new_id(),
            }
        } else {
            callee
        }
    }

    fn primary(&mut self) -> Expr {
        if self.try_consume(TokenType::True) {
            return Expr::Literal {
                repr: self.take_prev(),
                val: Value::Bool(true),
                id: self.new_id(),
            };
        } else if self.try_consume(TokenType::False) {
            return Expr::Literal {
                repr: self.take_prev(),
                val: Value::Bool(false),
                id: self.new_id(),
            };
        } else if self.try_consume(TokenType::None) {
            return Expr::Literal {
                repr: self.take_prev(),
                val: Value::None,
                id: self.new_id(),
            };
        } else if self.try_consume(TokenType::IntLiteral) {
            let mut literal = self.take_prev();
            let value: Result<i64, ParseError> = literal
                .take_lexeme()
                .unwrap()
                .parse()
                .map_err(|_e| ParseError::ParseIntError);
            match value {
                Ok(i) => {
                    return Expr::Literal {
                        repr: literal,
                        val: Value::Int(i),
                        id: self.new_id(),
                    };
                }
                Err(e) => {
                    self.error_at(&literal, e);
                    return Expr::dummy();
                }
            }
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
                        val: Value::Float(f),
                        id: self.new_id(),
                    };
                }
                Err(e) => {
                    self.error_at(&literal, e);
                    return Expr::dummy();
                }
            }
        } else if self.try_consume(TokenType::StringLiteral) {
            let mut literal = self.take_prev();
            let value = literal.take_lexeme().unwrap();
            let value = Value::from(value[1..value.len() - 1].to_owned()); // trim quotes
            return Expr::Literal {
                repr: literal,
                val: value,
                id: self.new_id(),
            };
        } else if self.try_consume(TokenType::CharLiteral) {
            let mut literal = self.take_prev();
            let lexeme = literal.take_lexeme().unwrap();
            let mut chars = lexeme.chars();
            chars.next(); // Consume opening quote
            let value = Value::Char(chars.next().unwrap());
            return Expr::Literal {
                repr: literal,
                val: value,
                id: self.new_id(),
            };
        } else if self.try_consume(TokenType::OpenParen) {
            let old = self.ignore_newlines;
            self.ignore_newlines = true;
            let expr = self.expression();
            self.ignore_newlines = old;
            self.expect_because(TokenType::CloseParen, ParseError::ParenNotClosed);
            return expr;
        } else if self.try_consume(TokenType::Identifier) {
            let identifier = self.take_prev();
            return Expr::Variable {
                identifier,
                id: self.new_id(),
            };
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
                && *prev.kind() == TokenType::Semicolon
            {
                self.advance();
                return;
            }
            match self.peek().kind() {
                // TODO other statement starts
                TokenType::New
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

    fn new_id(&mut self) -> usize {
        self.next_id += 1;
        self.next_id
    }

    fn expect_binding(&mut self) -> Token {
        self.expect_because(TokenType::Identifier, ParseError::NotAssignTarget);
        self.take_prev()
    }

    fn expect_type(&mut self) -> ValueType {
        self.skip_newlines();
        if self.try_consume_any(&VALUE_TYPES) {
            let token = self.take_prev();
            ValueType::from(*token.kind()).unwrap()
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
        *self.peek().kind() == kind
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
        *self.peek().kind() == TokenType::EOF
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

        self.first_error = Some(reason.clone());
    }
}

impl<'a> From<&'a str> for Parser<'a> {
    fn from(value: &'a str) -> Self {
        Self::from(Lexer::new(value))
    }
}

#[derive(Debug, Copy, Clone)]
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
        assert!(tester.check(TokenType::IntLiteral));
        assert!(tester.try_consume(TokenType::IntLiteral)); // advances to 2
        //  [1][2][3]
        //  (1) ^
        assert_eq!(tester.cur, make_token("2", 1));
        assert_eq!(tester.prev, Some(make_token("1", 1)));
        assert_eq!(tester.take_prev(), make_token("1", 1));
        assert_eq!(tester.prev, None);
        tester.expect_because(TokenType::IntLiteral, ParseError::NoExpression); // advances to 3
        assert_eq!(tester.prev, Some(make_token("2", 1)));
        tester.advance(); // advances past 3
        //  [1] [2] [3] EOF EOF EOF...
        //          (3)  ^
        assert!(tester.at_end());
        assert_eq!(*tester.cur.kind(), TokenType::EOF);
        assert_eq!(tester.prev, Some(make_token("3", 1)));
    }

    #[test]
    fn literals() {
        {
            let one = Parser::parse_expr_string("1").unwrap();
            let Expr::Literal { repr: _, val, .. } = one else {
                panic!("'one' did not match the pattern.")
            };
            let Value::Int(i) = val else {
                panic!("'i' did not match the pattern.")
            };
            assert_eq!(i, 1);
        }
        {
            let pi = Parser::parse_expr_string("3.14159").unwrap();
            let Expr::Literal { repr: _, val, .. } = pi else {
                panic!("'pi' did not match the pattern.")
            };
            let Value::Float(f) = val else {
                panic!("'i' did not match the pattern.")
            };
            assert_eq!(f, 3.14159)
        }
    }

    mod stateless {
        use super::*;

        #[test]
        fn exprs() {
            let mut expr = Parser::parse_expr_string("1+1").unwrap();
            let mut ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(2)));

            expr = Parser::parse_expr_string("5 + 3 * 2").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(11)));

            expr = Parser::parse_expr_string("10 - 4 / 2").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(8)));

            expr = Parser::parse_expr_string("(8 + 2) * (3 - 1)").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(20)));

            expr = Parser::parse_expr_string("-5 + 3").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(-2)));

            expr = Parser::parse_expr_string("!true").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Bool(false)));

            expr = Parser::parse_expr_string("~15").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(!15)));

            expr = Parser::parse_expr_string("7 & 3").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(3)));

            expr = Parser::parse_expr_string("12 | 5").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(13)));

            expr = Parser::parse_expr_string("9 ^ 6").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(15)));

            expr = Parser::parse_expr_string("4 << 2").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(16)));

            expr = Parser::parse_expr_string("16 >> 1").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(8)));

            expr = Parser::parse_expr_string("-(-10)").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(10)));

            expr = Parser::parse_expr_string("!(false)").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Bool(true)));

            expr = Parser::parse_expr_string("~~7").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(7)));

            expr = Parser::parse_expr_string("(5 + 3) & (2 * 4)").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(8)));

            expr = Parser::parse_expr_string("100 / 10 + 5 * 2").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(20)));

            expr = Parser::parse_expr_string("(20 - 5) * (3 + 2) / 5").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(15)));

            expr = Parser::parse_expr_string("14 % 3").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(2)));

            expr = Parser::parse_expr_string("-10 % 3").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(2)));

            expr = Parser::parse_expr_string("5.5 + 2.5").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Float(8.0)));

            expr = Parser::parse_expr_string("10.0 / 4.0").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Float(2.5)));

            expr = Parser::parse_expr_string("3 + 4.5").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Float(7.5)));

            expr = Parser::parse_expr_string("2.5 * 2").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Float(5.0)));

            expr = Parser::parse_expr_string("!(5 > 3)").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Bool(false)));

            expr = Parser::parse_expr_string("10 & ~5").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(10)));

            expr = Parser::parse_expr_string("(8 << 1) | (4 >> 1)").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(18)));

            expr = Parser::parse_expr_string("-5 * -3").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(15)));
        }

        #[test]
        fn newline_exprs() {
            let mut expr;
            let mut ans;

            expr = Parser::parse_expr_string("5 +\n3").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(5 + 3)));

            expr = Parser::parse_expr_string("10 -\n4 /\n2").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(10 - 4 / 2)));

            expr = Parser::parse_expr_string("(8 +\n2) * (3\n- 1)").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int((8 + 2) * (3 - 1))));

            expr = Parser::parse_expr_string("-5\n+ 3").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(-5))); // newline should ignore the rest

            expr = Parser::parse_expr_string("-5 \\ \n+ 3").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(-5 + 3))); // backslash ignores newline

            expr = Parser::parse_expr_string("!true\n|| false").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Bool(!true || false)));

            expr = Parser::parse_expr_string("7 &\n3").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(7 & 3)));

            expr = Parser::parse_expr_string("12 |\n5\n^ 6").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(12 | 5)));

            // 12 |
            // (5
            //    ^ 6)
            expr = Parser::parse_expr_string("12 |\n(5\n^ 6)").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(12 | (5 ^ 6))));

            expr = Parser::parse_expr_string("12 |\n5 ^\n6").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(12 | 5 ^ 6)));

            expr = Parser::parse_expr_string("4 << \n 2").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(4 << 2)));

            expr = Parser::parse_expr_string("~15\n& 7").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(!15)));

            expr = Parser::parse_expr_string("(~15\n& 7)").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(!15 & 7)));

            expr = Parser::parse_expr_string("-(-10)\n+ 5").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(-(-10))));

            expr = Parser::parse_expr_string("!(false)\n&& true").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Bool(!(false) && true)));

            expr = Parser::parse_expr_string("(5 +\n3) &\n(2\n*\n4)").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int((5 + 3) & (2 * 4))));

            expr = Parser::parse_expr_string("100\n/\n10\n+\n5 * 2").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(100)));

            expr = Parser::parse_expr_string("(100\n/\n10\n+\n5 * 2)").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(100 / 10 + 5 * 2)));

            expr = Parser::parse_expr_string("1 == 0 ? 1\n : 2").unwrap();
            ans = evaluate_static(&expr);
            assert!(matches!(ans, Ok(Value::Int(2))));

            expr = Parser::parse_expr_string("true ? 5\n: 10").unwrap();
            ans = evaluate_static(&expr);
            assert_eq!(ans, Ok(Value::Int(5)));
        }

        #[test]
        fn if_stmts() {
            // TODO
        }
    }
}
