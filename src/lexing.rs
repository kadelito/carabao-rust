use std::fmt::{Debug, Display};

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
    line: u32,
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
            TokenType::DecIntLiteral
            | TokenType::HexIntLiteral
            | TokenType::BinIntLiteral
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
                        loc: TokenLocation {
                            line: self.line,
                        },
                        error: None
                    },
                    Err(e) => Token {
                        kind: TokenType::Error,
                        lexeme: None,
                        loc: TokenLocation {
                            line: self.line,
                        },
                        error: Some(e)
                    },
                }
            }
            _ => Token {
                kind,
                lexeme: None,
                loc: TokenLocation {
                    line: self.line,
                },
                error: None
            },
        }
    }

    fn token_with_lexeme(&self, kind: TokenType, lexeme: String) -> Token {
        Token {
            kind,
            lexeme: Some(lexeme),
            loc: TokenLocation {
                line: self.line,
            },
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
        let c1 = chars.next().unwrap(); // we know length >= 1
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
						't' =>  
							if let Some(c3) = chars.next() {
								match c3 {
									'r' =>  
										if let Some(c4) = chars.next() {
											match c4 {
												'i' =>  Lexer::match_keyword(&lexeme, "string", T::String, 4),
												'u' =>  Lexer::match_keyword(&lexeme, "struct", T::Struct, 4),
												_ => T::Identifier
											}
										} else { T::Identifier }
									_ => T::Identifier
								}
							} else { T::Identifier }
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
                if self.advance() == '\\' {
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
    
    fn number(&mut self, starts_with_0: bool) -> Token {

        // 0 already consumed
        let base = if !starts_with_0 {
            10
        } else if self.try_consume('x') {
            16   
        } else if self.try_consume('b') {
            2
        } else {
            // just 0, next loop won't run
            10
        };

        while self.peek().is_digit(base) {
            self.advance();
        }
        // handles the 10.method() case
        let is_float = self.peek() == '.' && self.peek_ahead(1).is_ascii_digit();
        let kind;
        if is_float {
            self.advance(); // consume '.'
            while self.peek().is_ascii_hexdigit() {
                self.advance();
            }
            kind = TokenType::FloatLiteral;
        } else {
            kind = match base {
                10 => TokenType::DecIntLiteral,
                16 => TokenType::HexIntLiteral,
                2  => TokenType::BinIntLiteral,
                _ => unreachable!()
            };
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
        pairs: &[(&str, TokenType)],
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
            return self.make_token(*kind);
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
            return self.number(c == '0');
            return self.number(c == '0');
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
                let char = self.advance(); // the char itself
                if self.try_consume('\'') {
                    self.make_token(TokenType::CharLiteral)
                } else if char == '\\' {
                    self.advance(); // escaped character
                    if !self.try_consume('\'') {
                        self.error_token(TokenizationError::UnterminatedChar)
                    } else {
                        self.make_token(TokenType::CharLiteral)
                    }
                } else {
                    println!("'");
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
    lexeme: Option<String>,
    loc: TokenLocation,
    error: Option<TokenizationError>,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct TokenLocation {
    pub line: u32
}

impl TokenLocation {
    pub fn is_before(&self, other: &Self) -> bool {
        self.line < other.line
    }
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
    pub fn kind(&self) -> TokenType {
        self.kind
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

    pub fn lexeme(&self) -> &String {
        self.lexeme.as_ref().expect("This token's lexeme should not have been taken yet")
    }

    pub fn line(&self) -> u32 {
        self.loc.line
    }

    pub fn location(&self) -> TokenLocation {
        self.loc
    }

    pub fn error(&self) -> Option<TokenizationError> {
        self.error
    }
}

impl Default for Token {
    fn default() -> Self {
        Self {
            kind: TokenType::Error,
            lexeme: None,
            loc: TokenLocation {
                line: u32::MAX,
            },
            error: Some(TokenizationError::EmptyToken)
        }
    }
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", {
            use TokenType as T;
            if let Some(str) = &self.lexeme {
                str.to_owned()
            } else {
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
                    T::Struct => "struct",
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
                    T::DecIntLiteral => "[Int-10]",
                    T::HexIntLiteral => "[Int-16]",
                    T::BinIntLiteral => "[Int-2]",
                    T::FloatLiteral => "[Float]",
                    T::CharLiteral => "[Char]",
                    T::StringLiteral => "[String]",
                    T::Identifier => "[Identifier]",
                    T::Error => "[Error]",
                    T::EOF => "[EOF]",
                }
                .to_owned()
            }
        })
    }
}

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
    DecIntLiteral,
    HexIntLiteral,
    BinIntLiteral,
    FloatLiteral,
    CharLiteral,
    StringLiteral,
    Identifier,

    // Keywords
    // TODO class/struct
    // Class, // im hesitant abt this one
    Struct,
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
    Return,
    Break,
    Continue,
    True,
    False,
    None,
    // Try,
    // Catch,

    // Misc
    Error,
    EOF,
}