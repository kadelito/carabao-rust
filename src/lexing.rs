use std::fmt::Debug;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum TokenizationError {
    Utf8Error,
    UnterminatedString,
    UnterminatedChar,
    UnexpectedChar,
    EmptyToken,
}

pub struct Lexer<'a> {
    chars: &'a [u8],
    cur: usize,
    start: usize,
    line: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Lexer {
            chars: src.as_bytes(),
            cur: 0,
            start: 0,
            line: 1,
        }
    }

    pub fn from_str(src: &'a str) -> Vec<Token> {
        Lexer::new(src).to_vec()
    }

    pub fn tokenize(src: &str) -> Vec<Token> {
        let lexer = Lexer::new(&src);
        lexer.to_vec()
    }

    /// Returns the character about to be consumed.
    /// If the character to be consumed is at an index >= length,
    /// returns '\0'
    fn peek(&self) -> char {
        if self.at_end() {
            '\0'
        } else {
            self.chars[self.cur] as char
        }
    }

    /// Returns the character 1 index ahead.
    /// If 1 index ahead is out of bounds, returns '\0'
    fn peek_ahead(&self, offset: usize) -> char {
        if self.cur + offset >= self.chars.len() {
            '\0'
        } else {
            self.chars[self.cur + offset] as char
        }
    }

    /// Returns the substring from the start offset inclusive to the end offset exclusive.
    ///
    /// `peek_substring(0, 1)` should be a 1-length string whose char is `peek()`
    fn peek_substring(&self, start_offset: usize, end_offset: usize) -> &str {
        if end_offset <= start_offset {
            ""
        }
        // note that we allow cur + end_offset == chars.len
        else if self.cur + end_offset > self.chars.len() {
            ""
        } else {
            let (start, end) = (self.cur + start_offset, self.cur + end_offset);
            match str::from_utf8(&self.chars[start..end]) {
                Ok(s) => s,
                Err(_) => "",
            }
        }
    }

    /**
    Returns the character that was just passed/consumed.
    ```
    let c1 = peek();
    let c2 = advance();
    assert_eq!(c1, c2);
    ```
    */
    fn advance(&mut self) -> char {
        let c = self.peek();
        self.cur += 1;
        c
    }

    fn at_end(&self) -> bool {
        self.cur >= self.chars.len()
    }

    /// Tries to consume a character, advancing if the character was found.
    fn try_consume(&mut self, expected: char) -> bool {
        if self.at_end() {
            return false;
        }
        if self.peek() != expected {
            return false;
        }
        self.advance();
        true
    }

    fn get_lexeme(&self) -> Result<&str, TokenizationError> {
        str::from_utf8(&self.chars[self.start..self.cur]).map_err(|_e| TokenizationError::Utf8Error)
    }

    fn make_token(&self, kind: TokenType) -> Token {
        match kind {
            TokenType::IntLiteral
            | TokenType::FloatLiteral
            | TokenType::CharLiteral
            | TokenType::StringLiteral
            | TokenType::Identifier
            | TokenType::Error => {
                let lexeme = self.get_lexeme();
                match lexeme {
                    Ok(lexeme) => Token {
                        kind,
                        lexeme: Some(lexeme.to_owned()),
                        line: self.line,
                        error: None
                    },
                    Err(e) => Token {
                        kind: TokenType::Error,
                        lexeme: None,
                        line: self.line,
                        error: Some(e)
                    },
                }
            }
            _ => Token {
                kind,
                lexeme: None,
                line: self.line,
                error: None
            },
        }
    }

    fn token_with_lexeme(&self, kind: TokenType, lexeme: String) -> Token {
        Token {
            kind,
            lexeme: Some(lexeme),
            line: self.line,
            error: None
        }
    }

    fn error_token(&self, error: TokenizationError) -> Token {
        let mut token = self.make_token(TokenType::Error);
        token.error = Some(error);
        token
    }

    /// Ends with the lexer pointing at a non-ignored character.
    ///
    /// This does not include newline characters, as they are parsed as tokens.
    fn skip_ignored(&mut self) {
        loop {
            if self.at_end() {
                return;
            }
            let c = self.peek();
            match c {
                // Ignored whitespace
                ' ' | '\r' | '\t' => {
                    self.advance();
                }

                // Forward slash could be a comment
                '/' => {
                    if self.peek_ahead(1) == '/' {
                        // Single-line comment
                        while !self.at_end() && self.peek() != '\n' {
                            self.advance();
                        }
                    } else if self.peek_ahead(1) == '*' {
                        while !self.at_end() && self.peek_substring(0, 2) != "*/" {
                            self.advance();
                        }
                        // Skip */
                        self.advance();
                        self.advance();
                        // i dont really care about unterminated comments
                    } else {
                        // a single forward slash
                        return;
                    }
                }

                // Not ignored
                _ => {
                    return;
                }
            }
        }
    }

    fn match_keyword(lexeme: &str, keyword: &str, kind: TokenType, start: usize) -> TokenType {
        if lexeme[start..] == keyword[start..] {
            kind
        } else {
            TokenType::Identifier
        }
    }

    fn identifier(&mut self) -> Token {
        use TokenType as T;
        while !self.at_end() && (self.peek().is_ascii_alphanumeric() || self.peek() == '_') {
            self.advance();
        }
        let lexeme = self.get_lexeme().unwrap().to_owned(); // only ascii characters during construction
        let mut chars = lexeme.chars();

        // Trie for keywords
        let c1 = chars.next().unwrap(); // must have length >= 1
        let kind = match c1 {
			'a' =>  
				if let Some(c2) = chars.next() {
					match c2 {
						'n' =>  Lexer::match_keyword(&lexeme, "any", T::Any, 2),
						's' =>  Lexer::match_keyword(&lexeme, "as", T::As, 2),
						_ => T::Identifier
					}
				} else { T::Identifier }
			'b' =>  
				if let Some(c2) = chars.next() {
					match c2 {
						'o' =>  Lexer::match_keyword(&lexeme, "bool", T::Bool, 2),
						'r' =>  Lexer::match_keyword(&lexeme, "break", T::Break, 2),
						_ => T::Identifier
					}
				} else { T::Identifier }
			'c' =>  
				if let Some(c2) = chars.next() {
					match c2 {
						'h' =>  Lexer::match_keyword(&lexeme, "char", T::Char, 2),
						'o' =>  Lexer::match_keyword(&lexeme, "continue", T::Continue, 2),
						_ => T::Identifier
					}
				} else { T::Identifier }
			'e' =>  Lexer::match_keyword(&lexeme, "else", T::Else, 1),
			'f' =>  
				if let Some(c2) = chars.next() {
					match c2 {
						'a' =>  Lexer::match_keyword(&lexeme, "false", T::False, 2),
						'l' =>  Lexer::match_keyword(&lexeme, "float", T::Float, 2),
						'o' =>  Lexer::match_keyword(&lexeme, "for", T::For, 2),
						'u' =>  Lexer::match_keyword(&lexeme, "func", T::Func, 2),
						_ => T::Identifier
					}
				} else { T::Identifier }
			'i' =>  
				if let Some(c2) = chars.next() {
					match c2 {
						'f' =>  Lexer::match_keyword(&lexeme, "if", T::If, 2),
						'n' =>  
							if let Some(c3) = chars.next() {
								match c3 {
									't' =>  Lexer::match_keyword(&lexeme, "int", T::Int, 3),
									_ => T::Identifier
								}
							} else { T::In } // chars match & length is 2
						_ => T::Identifier
					}
				} else { T::Identifier }
			'n' =>  
				if let Some(c2) = chars.next() {
					match c2 {
						'e' =>  Lexer::match_keyword(&lexeme, "new", T::New, 2),
						'o' =>  Lexer::match_keyword(&lexeme, "none", T::None, 2),
						_ => T::Identifier
					}
				} else { T::Identifier }
			'r' =>  Lexer::match_keyword(&lexeme, "return", T::Return, 1),
			's' =>  
				if let Some(c2) = chars.next() {
					match c2 {
						't' =>  Lexer::match_keyword(&lexeme, "string", T::String, 2),
						'u' =>  Lexer::match_keyword(&lexeme, "summon", T::Summon, 2),
						_ => T::Identifier
					}
				} else { T::Identifier }
			't' =>  Lexer::match_keyword(&lexeme, "true", T::True, 1),
			'w' =>  Lexer::match_keyword(&lexeme, "while", T::While, 1),
			_ => T::Identifier
		};
        self.token_with_lexeme(kind, lexeme)
    }

    // First quote has been consumed atp
    fn string(&mut self) -> Token {
        let triple_quotes = self.peek_substring(0, 2) == "\"\"";
        if triple_quotes {
            // Triple quotes, multiline string
            while !self.at_end() && self.peek_substring(0, 3) != "\"\"\"" {
                if self.peek() == '\n' {
                    self.line += 1;
                }
                self.advance();
                if self.peek() == '\\' {
                    self.advance();
                }
            }
            // Try to consume two quotes, handle error later
            if !self.at_end() {
                self.advance();
                self.advance();
            }
        } else {
            // Single-quote string
            while !self.at_end() && self.peek() != '"' && self.peek() != '\n' {
                self.advance();
                if self.peek() == '\\' {
                    self.advance(); // skip escaped quote. or newline
                }
            }
        }

        if self.peek() != '"' {
            // String did not end naturally
            // If ended by a newline, it is NOT consumed
            return self.error_token(TokenizationError::UnterminatedString);
        }

        self.advance(); // Consume end quote

        let token;
        if triple_quotes {
            // change start & cur to trim 2 quotes, pretend it's single-quoted
            self.start += 2;
            self.cur -= 2;
            token = self.make_token(TokenType::StringLiteral);
            self.cur += 2;
        } else {
            token = self.make_token(TokenType::StringLiteral);
        }
        token
    }
    
    // TODO non-decimal numbers (0xff, 0b1011, etc)
    fn number(&mut self) -> Token {
        while self.peek().is_ascii_digit() {
            self.advance();
        }
        let is_float = self.peek() == '.' && self.peek_ahead(1).is_ascii_digit();
        let kind;
        if is_float {
            self.advance(); // consume '.'
            while self.peek().is_ascii_hexdigit() {
                self.advance();
            }
            kind = TokenType::FloatLiteral;
        } else {
            kind = TokenType::IntLiteral;
        }
        self.make_token(kind)
    }

    fn one_or_two_char_token(&mut self, from_one: TokenType, pairs: Vec<(char, TokenType)>) -> Token {
        for (c, from_two) in pairs {
            if self.try_consume(c) {
                return self.make_token(from_two);
            }
        }
        self.make_token(from_one)
    }

    /// Tries to match multiple tokens
    /// that start with the char that was just consumed.
    /// 
    /// Each tuple in `pairs` has the rest of the string and the corresponding token.
    /// 
    /// For example, to consume `+`, `++`, or `+=`, you might do:
    /// ```
    /// use TokenType as T;
    /// match chars.next() {
    ///     /* ... */
    ///     '+' => self.one_or_more_char_token(T::Plus, vec![
    ///         ("=", T::PlusEqual),
    ///         ("+", T:DoublePlus)])
    /// }
    /// ```
    fn one_or_more_char_token(
        &mut self,
        from_one: TokenType,
        pairs: Vec<(&str, TokenType)>,
    ) -> Token {
        let token_start = self.cur;
        'next_token: for (s, kind) in pairs {
            for c in s.chars() {
                if !self.try_consume(c) {
                    // if ANY character doesn't match, restart with the next token
                    self.cur = token_start;
                    continue 'next_token;
                }
            }
            // we consumed the whole thing without breaking
            return self.make_token(kind);
        }
        self.make_token(from_one)
    }

    /// Scans ahead using the lexer's source, returning a token.
    ///
    /// After this method is called,
    /// the lexer should be pointing at whitespace or the start of the next token.
    pub fn scan_token(&mut self) -> Token {
        self.skip_ignored();
        self.start = self.cur;

        if self.at_end() {
            return self.make_token(TokenType::EOF);
        }

        let c = self.advance();

        if c.is_ascii_alphabetic() || c == '_' {
            return self.identifier();
        }

        if c.is_ascii_digit() {
            return self.number();
        }

        match c {
            // Single characters
            '(' => self.make_token(TokenType::OpenParen),
            ')' => self.make_token(TokenType::CloseParen),
            '{' => self.make_token(TokenType::OpenBrace),
            '}' => self.make_token(TokenType::CloseBrace),
            '[' => self.make_token(TokenType::OpenBracket),
            ']' => self.make_token(TokenType::CloseBracket),
            ',' => self.make_token(TokenType::Comma),
            '?' => self.make_token(TokenType::Question),
            ':' => self.make_token(TokenType::Colon),
            ';' => self.make_token(TokenType::Semicolon),
            '~' => self.make_token(TokenType::Tilde),
            '+' => self.make_token(TokenType::Plus),
            '-' => self.make_token(TokenType::Minus),
            '*' => self.make_token(TokenType::Star),
            '/' => self.make_token(TokenType::FSlash),
            '%' => self.make_token(TokenType::Percent),
            '^' => self.make_token(TokenType::Carrot),
            '\n' => {
                let token = self.make_token(TokenType::Newline);
                self.line += 1;
                token
            }
            
            // Single or double character
            '.' => self.one_or_two_char_token(TokenType::Dot, vec![('.', TokenType::DoubleDot)]),
            '!' => self.one_or_two_char_token(TokenType::Bang, vec![('=', TokenType::BangEqual)]),
            '=' => {
                self.one_or_two_char_token(TokenType::Equal, vec![('=', TokenType::DoubleEqual)])
            }
            '<' => self.one_or_two_char_token(
                TokenType::Less,
                vec![('=', TokenType::LessEqual), ('<', TokenType::DoubleLess)],
            ),
            '>' => self.one_or_two_char_token(
                TokenType::Greater,
                vec![('=', TokenType::GreaterEqual), ('>', TokenType::DoubleGreater),
                ],
            ),
            '&' => self.one_or_two_char_token(
                TokenType::Ampersand,
                vec![('&', TokenType::DoubleAmpersand)],
            ),
            '|' => self.one_or_two_char_token(
                TokenType::VertBar,
                vec![('|', TokenType::DoubleVertBar)]),

            '"' => self.string(),

            // Char literal
            // TODO escape characters here too
            '\'' => {
                self.advance(); // the char itself
                if self.try_consume('\'') {
                    self.make_token(TokenType::CharLiteral)
                } else {
                    self.error_token(TokenizationError::UnterminatedChar)
                }
            }

            // Backslash skips until next newline
            '\\' => {
                self.skip_ignored(); // comment or whitespace before newline
                if self.try_consume('\n') {
                    self.line += 1;
                    self.scan_token() // return the following token recursively
                } else {
                    // whoopsie, no newline token
                    let next_token = self.error_token(TokenizationError::UnexpectedChar);
                    while !self.at_end() && !self.try_consume('\n') {
                        self.advance();
                    }
                    next_token
                }
            }

            _ => self.error_token(TokenizationError::UnexpectedChar),
        }
    }

    fn to_vec(mut self) -> Vec<Token> {
        let mut vec = Vec::new();
        let mut token = self.scan_token();
        while !matches!(token.kind(), TokenType::EOF) {
            vec.push(token);
            token = self.scan_token();
        }
        vec
    }
}

//TODO use columns or indices
#[derive(PartialEq, Clone)]
pub struct Token {
    kind: TokenType,
    pub lexeme: Option<String>,
    line: usize,
    error: Option<TokenizationError>,
}

impl Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lexeme = match self.lexeme.as_ref() {
            Some(l) => format!("({})", l),
            None => String::new(),
        };
        write!(f, "{:?}{}", self.kind, lexeme)
    }
}

/// Getters for Token
impl Token {
    pub fn dummy() -> Self {
        Token {
            kind: TokenType::Error,
            lexeme: None,
            line: usize::MAX,
            error: Some(TokenizationError::EmptyToken)
        }
    }

    pub fn kind(&self) -> &TokenType {
        &self.kind
    }

    /// Panics if the lexeme has already been taken.
    /// 
    /// Intended for only identifiers,
    /// as it is expected for their lexemes to not be taken.
    pub fn copy_ident(&self) -> String {
        self.lexeme.as_ref().unwrap().clone()
    }

    pub fn take_lexeme(&mut self) -> Option<String> {
        self.lexeme.take()
    }

    pub fn lexeme(&self) -> Option<&String> {
        self.lexeme.as_ref()
    }

    pub fn line(&self) -> usize {
        self.line
    }

    pub fn error(&self) -> Option<TokenizationError> {
        self.error
    }

    pub fn to_string(&self) -> String {
        use TokenType as T;
        if let Some(str) = &self.lexeme {
            return str.to_owned();
        }
        match &self.kind {
            T::OpenParen => "(",
            T::CloseParen => ")",
            T::OpenBrace => "{",
            T::CloseBrace => "}",
            T::OpenBracket => "[",
            T::CloseBracket => "]",
            T::Dot => ".",
            T::Comma => ",",
            T::Question => "?",
            T::Colon => ":",
            T::Bang => "!",
            T::Tilde => "~",
            T::Plus => "+",
            T::Minus => "-",
            T::Star => "*",
            T::FSlash => "/",
            T::Percent => "%",
            T::Ampersand => "&",
            T::Carrot => "^",
            T::VertBar => "|",
            T::DoubleLess => "<<",
            T::DoubleGreater => ">>",
            T::Less => "<",
            T::Greater => ">",
            T::GreaterEqual => ">=",
            T::LessEqual => "<=",
            T::BangEqual => "!=",
            T::DoubleEqual => "==",
            T::DoubleAmpersand => "&&",
            T::DoubleVertBar => "||",
            T::DoubleDot => "..",
            T::Equal => "=",
            T::Newline => "\\n",
            T::Semicolon => ";",
            T::Summon => "import",
            T::As => "as",
            T::In => "in",
            T::Func => "func",
            T::If => "if",
            T::Else => "else",
            T::For => "for",
            T::While => "while",
            T::Return => "return",
            T::Break => "break",
            T::Continue => "continue",
            T::True => "true",
            T::False => "false",
            T::None => "none",
            T::New => "auto",
            T::Any => "any",
            T::Int => "int",
            T::Bool => "bool",
            T::Float => "float",
            T::Char => "char",
            T::String => "string",
            T::IntLiteral => "[Int]",
            T::FloatLiteral => "[Float]",
            T::CharLiteral => "[Char]",
            T::StringLiteral => "[String]",
            T::Identifier => "[Identifier]",
            T::Error => "[Error]",
            T::EOF => "[EOF]",
        }
        .to_owned()
    }
}

// TODO Implement the commented-out tokens
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum TokenType {
    // Single character
    OpenParen,
    CloseParen,
    OpenBrace,
    CloseBrace,
    OpenBracket,
    CloseBracket,
    Newline,
    Semicolon,
    Dot,
    Comma,
    Question,
    Colon,
    Equal,
    Plus,
    Minus,
    Star,
    FSlash,
    Percent,
    Tilde,
    Ampersand,
    Carrot,
    VertBar,
    Bang,
    Less,
    Greater,

    // Two or more character
    // TODO assignment operators, have fun testing :)
    // DoublePlus, DoubleMinus,
    // PlusEqual, MinusEqual, StarEqual, FSlashEqual, PercentEqual,
    // AmpersandEqual, CarrotEqual, VertBarEqual,
    // DoubleLessEqual, DoubleGreaterEqual,
    // DoubleAmpersandEqual, DoubleVertBarEqual,
    DoubleLess,
    DoubleGreater,
    DoubleAmpersand,
    DoubleVertBar,
    GreaterEqual,
    LessEqual,
    BangEqual,
    DoubleEqual,
    DoubleDot,

    // Literals
    IntLiteral,
    FloatLiteral,
    CharLiteral,
    StringLiteral,
    Identifier,

    // Keywords
    // Class, // im hesitant abt this one
    // Struct,
    New,
    Any,
    Int,
    Float,
    Char,
    Bool,
    String,
    Summon,
    As,
    In,
    Func,
    If,
    Else,
    For,
    While,
    // Try,
    // Catch,
    Return,
    Break,
    Continue,
    True,
    False,
    None,

    // Misc
    Error,
    EOF,
}

#[cfg(test)]
pub mod lexing_tests {
    use crate::lexing::*;

    pub fn make_token(src: &str, line: usize) -> Token {
        let mut token = Lexer::new(src).scan_token();
        token.line = line;
        token
    }

    #[test]
    fn identifier_1() {
        let tester = Lexer::new("hello world!");
        let tokens = tester.to_vec();

        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, TokenType::Identifier);
        assert_eq!(tokens[1].kind, TokenType::Identifier);
        assert_eq!(tokens[2].kind, TokenType::Bang);
    }

    #[test]
    fn newlines_1() {
        let tester = Lexer::new("  \t \n \\  \n");
        let tokens = tester.to_vec();

        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenType::Newline);
    }

    #[test]
    fn newlines_2() {
        // should get parsed as [a] [+] [b] [\n] [c] [-] [d]
        let tester = Lexer::new(
            "a \\
+\\
b
c - d \\
",
        );
        let tokens = tester.to_vec();
        let types: Vec<&TokenType> = tokens.iter().map(|tkn| tkn.kind()).collect();

        assert_eq!(types.len(), 7);

        assert_eq!(
            types,
            vec![
                &TokenType::Identifier,
                &TokenType::Plus,
                &TokenType::Identifier,
                &TokenType::Newline,
                &TokenType::Identifier,
                &TokenType::Minus,
                &TokenType::Identifier,
            ]
        )
    }

    #[test]
    fn peekaboo() {
        let mut tester = Lexer::new("0123456");
        // [0][1][2][3][4][5][6]
        //  ^
        assert_eq!(tester.peek(), '0');
        assert_eq!(tester.peek_ahead(1), '1');
        assert_eq!(tester.peek_ahead(2), '2');
        assert_eq!(tester.peek_ahead(200), '\0');
        assert_eq!(tester.peek_substring(1, 3), "12");
        assert_eq!(
            tester.peek_substring(3, 4).chars().next().unwrap(),
            tester.peek_ahead(3)
        );
        assert_eq!(tester.peek_substring(1, 10), "");
        tester.advance();
        tester.advance();
        // [0][1][2][3][4][5][6]
        //        ^  1  2  3  4
        assert_eq!(tester.peek(), '2');
        assert_eq!(tester.peek_ahead(1), '3');
        assert_eq!(tester.peek_ahead(2), '4');
        assert_eq!(tester.peek_ahead(5), '\0');
        assert_eq!(tester.peek_ahead(6), '\0');
        assert_eq!(tester.peek_substring(0, 4), "2345");
        assert_eq!(tester.peek_substring(1, 5), "3456");
        assert_eq!(tester.peek_substring(2, 6), "");
    }

    #[test]
    fn double_chars() {
        let mut tester = Lexer::new("=====");
        //                                          == == =
        assert_eq!(tester.scan_token().kind, TokenType::DoubleEqual);
        assert_eq!(tester.scan_token().kind, TokenType::DoubleEqual);
        assert_eq!(tester.scan_token().kind, TokenType::Equal);
        assert_eq!(tester.scan_token().kind, TokenType::EOF);

        tester = Lexer::new(">>>==<=!!=!");
        //                     >> >= = <= ! != !
        assert_eq!(tester.scan_token().kind, TokenType::DoubleGreater);
        assert_eq!(tester.scan_token().kind, TokenType::GreaterEqual);
        assert_eq!(tester.scan_token().kind, TokenType::Equal);
        assert_eq!(tester.scan_token().kind, TokenType::LessEqual);
        assert_eq!(tester.scan_token().kind, TokenType::Bang);
        assert_eq!(tester.scan_token().kind, TokenType::BangEqual);
        assert_eq!(tester.scan_token().kind, TokenType::Bang);
        assert_eq!(tester.scan_token().kind, TokenType::EOF);
    }

    #[test]
    fn keywords() {
        
    }

    #[test]
    fn not_keywords() {
        let mut tester = Lexer::new(
            "ints floats chars bools strings vars imports ass ins funcs ifs elses fors whiles returns breaks continues trues falses"
        );
        for _ in 1..=19 {
            assert_eq!(tester.scan_token().kind, TokenType::Identifier);
        }
        assert_eq!(tester.scan_token().kind, TokenType::EOF);

        tester = Lexer::new("boo brea cha continu els fals floa fo fun i impor retur tru whil");
        let mut token = tester.scan_token();
        while { token = tester.scan_token(); token.kind } != TokenType::EOF {
            assert_eq!(token.kind, TokenType::Identifier);
        }
    }

    #[test]
    fn errors() {
        /*
        Utf8Error,
        UnterminatedString,
        UnterminatedChar,
        UnexpectedChar,
        */
        let s = "\':\' \': ` \"\n \\ ?";
        let mut tester = Lexer::new(&s);
        tester = Lexer::new(&s);
        assert_eq!(tester.scan_token().kind, TokenType::CharLiteral);
        assert_eq!(
            tester.scan_token().error,
            Some(TokenizationError::UnterminatedChar)
        );
        assert_eq!(
            tester.scan_token().error,
            Some(TokenizationError::UnexpectedChar)
        );
        assert_eq!(
            tester.scan_token().error,
            Some(TokenizationError::UnterminatedString)
        );
        assert_eq!(tester.scan_token().kind, TokenType::Newline);
        assert_eq!(
            tester.scan_token().error,
            Some(TokenizationError::UnexpectedChar)
        );
        assert_eq!(tester.scan_token().kind, TokenType::EOF);
    }
}
