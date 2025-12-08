keywords = """
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
    None
""".split(",")
keywords = [kw.strip() for kw in keywords]

def test_keywords(keywords: List[str]):
    lower_keywords = [kw.lower() for kw in keywords]
    exact_test_str = f'"{" ".join(lower_keywords)}"'
    print(     "    #[test]")
    print(     "    fn exact_keywords() {")
    print(    f"        let mut tester = Lexer::new({exact_test_str});")
    for kw in keywords:
        print(f"        assert_eq!(tester.scan_token().kind, TokenType::{kw});")
    print(    f"        assert_eq!(tester.scan_token().kind, TokenType::EOF);")
    print(     "    }")
    print(     "    ")

    almost_keywords_1 = [kw[:-1] for kw in lower_keywords]
    # filter out when 'int' -> 'in', which is already a keyword
    almost_keywords_1 = list(filter(lambda akw: akw not in lower_keywords, almost_keywords_1))
    almost_test_str = f'"{" ".join(almost_keywords_1)}"'
    print( "    #[test]")
    print( "    fn keywords_too_short() {")
    print(f"        let mut tester = Lexer::new({almost_test_str});")
    print(f"        for _ in 1..={len(almost_keywords_1)}")
    print(f"            assert_eq!(tester.scan_token().kind, TokenType::Identifier);")
    print( "        }")
    print(f"        assert_eq!(tester.scan_token().kind, TokenType::EOF);")
    print( "    }")
    print( "    ")

    almost_keywords_2 = [kw + "s" for kw in lower_keywords]
    # see above, overlap less likely but possible
    almost_keywords_2 = list(filter(lambda akw: akw not in lower_keywords, almost_keywords_2))
    almost_test_str_2 = f'"{" ".join(almost_keywords_2)}"'
    print( "    #[test]")
    print( "    fn keywords_too_long() {")
    print(f"        let mut tester = Lexer::new({almost_test_str_2});")
    print(f"        for _ in 1..={len(almost_keywords_2)}")
    print(f"            assert_eq!(tester.scan_token().kind, TokenType::Identifier);")
    print( "        }")
    print(f"        assert_eq!(tester.scan_token().kind, TokenType::EOF);")
    print( "    }")

test_keywords(keywords)