use relic::lexer::Lexer;
use relic::lexer::Number;
use relic::lexer::TokenType;

#[test]
fn param() {
    assert_eq!(
        Lexer::new("(())").collect::<Vec<TokenType>>(),
        vec![
            TokenType::LParem,
            TokenType::LParem,
            TokenType::RParem,
            TokenType::RParem
        ]
    )
}

#[test]
fn numeric() {
    assert_eq!(
        Lexer::new("123456").collect::<Vec<TokenType>>(),
        vec![TokenType::Number(Number::Int(123456))]
    )
}

#[test]
fn empty_input() {
    assert_eq!(Lexer::new("").collect::<Vec<TokenType>>(), vec![]);
}

#[test]
fn whitespace_only() {
    assert_eq!(Lexer::new("   \n\t  ").collect::<Vec<TokenType>>(), vec![]);
}

#[test]
fn comment() {
    assert_eq!(
        Lexer::new("1 ; 2 \n\t  3 ").collect::<Vec<TokenType>>(),
        vec![
            TokenType::Number(Number::Int(1)),
            TokenType::Number(Number::Int(3))
        ]
    );
}

#[test]
fn string() {
    assert_eq!(
        Lexer::new("\"a b c\n d\" ; \" e f\" \n\t  \"\" ").collect::<Vec<TokenType>>(),
        vec![
            TokenType::Symbol("a b c\n d".into()),
            TokenType::Symbol("".into())
        ]
    );
}

#[test]
fn quote_token() {
    assert_eq!(
        Lexer::new("'(1 2 3)").collect::<Vec<TokenType>>(),
        vec![
            TokenType::Quote,
            TokenType::LParem,
            TokenType::Number(Number::Int(1)),
            TokenType::Number(Number::Int(2)),
            TokenType::Number(Number::Int(3)),
            TokenType::RParem
        ]
    );
}

#[test]
fn keyword_def() {
    assert_eq!(
        Lexer::new("(def foo 42)").collect::<Vec<TokenType>>(),
        vec![
            TokenType::LParem,
            TokenType::Symbol("def".into()),
            TokenType::Symbol("foo".into()),
            TokenType::Number(Number::Int(42)),
            TokenType::RParem
        ]
    );
}

#[test]
fn keyword_set() {
    assert_eq!(
        Lexer::new("(set foo 42)").collect::<Vec<TokenType>>(),
        vec![
            TokenType::LParem,
            TokenType::Symbol("set".into()),
            TokenType::Symbol("foo".into()),
            TokenType::Number(Number::Int(42)),
            TokenType::RParem
        ]
    );
}

#[test]
fn keyword_lambda() {
    assert_eq!(
        Lexer::new("(lambda (x) (+ x 1))").collect::<Vec<TokenType>>(),
        vec![
            TokenType::LParem,
            TokenType::Symbol("lambda".into()),
            TokenType::LParem,
            TokenType::Symbol("x".into()),
            TokenType::RParem,
            TokenType::LParem,
            TokenType::Symbol("+".into()),
            TokenType::Symbol("x".into()),
            TokenType::Number(Number::Int(1)),
            TokenType::RParem,
            TokenType::RParem
        ]
    );
}

#[test]
fn symbol_token() {
    assert_eq!(
        Lexer::new("foo").collect::<Vec<TokenType>>(),
        vec![TokenType::Symbol("foo".into())]
    );
}

#[test]
fn mixed_tokens() {
    assert_eq!(
        Lexer::new("(lambda (x) (def foo x))").collect::<Vec<TokenType>>(),
        vec![
            TokenType::LParem,
            TokenType::Symbol("lambda".into()),
            TokenType::LParem,
            TokenType::Symbol("x".into()),
            TokenType::RParem,
            TokenType::LParem,
            TokenType::Symbol("def".into()),
            TokenType::Symbol("foo".into()),
            TokenType::Symbol("x".into()),
            TokenType::RParem,
            TokenType::RParem
        ]
    );
}

#[test]
fn symbol_with_numbers() {
    assert_eq!(
        Lexer::new("abc123").collect::<Vec<TokenType>>(),
        vec![TokenType::Symbol("abc123".into())]
    );
}

#[test]
fn multiple_whitespace() {
    assert_eq!(
        Lexer::new("(  1   2 )").collect::<Vec<TokenType>>(),
        vec![
            TokenType::LParem,
            TokenType::Number(Number::Int(1)),
            TokenType::Number(Number::Int(2)),
            TokenType::RParem
        ]
    );
}

#[test]
fn dot() {
    assert_eq!(
        Lexer::new("(a . b)").collect::<Vec<TokenType>>(),
        vec![
            TokenType::LParem,
            TokenType::Symbol("a".into()),
            TokenType::Dot,
            TokenType::Symbol("b".into()),
            TokenType::RParem
        ]
    );
}

#[test]
fn unknown_characters() {
    assert_eq!(
        Lexer::new("$").collect::<Vec<TokenType>>(),
        vec![TokenType::Symbol("$".into())]
    );
}
