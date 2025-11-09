use relic::lexer::LexerMonad;
use relic::lexer::Token;
use relic::number::Number;

fn get_tokens(mut l: LexerMonad<()>) -> Vec<Token> {
    let mut tokens = vec![];
    loop {
        match l.next_token() {
            Ok(new_l) => {
                tokens.push(new_l.get().clone());
                l = new_l.get_fp(); // Update l for the next iteration
            }
            Err(e) => {
                println!("{e}");
                break;
            }
        }
    }
    tokens
}

#[test]
fn param() {
    let tokens = LexerMonad::new_unnamed("(())");
    assert_eq!(
        get_tokens(tokens),
        vec![Token::LParem, Token::LParem, Token::RParem, Token::RParem]
    )
}

#[test]
fn numeric() {
    let tokens = LexerMonad::new_unnamed("123456 123.456 -123 -4.56");
    assert_eq!(
        get_tokens(tokens),
        vec![
            Token::Number(Number::Int(123456)),
            Token::Number(Number::Float(123.456)),
            Token::Number(Number::Int(-123)),
            Token::Number(Number::Float(-4.56)),
        ]
    )
}

#[test]
fn empty_input() {
    assert_eq!(get_tokens(LexerMonad::new_unnamed("")), vec![]);
}

#[test]
fn whitespace_only() {
    assert_eq!(get_tokens(LexerMonad::new_unnamed("   \n\t  ")), vec![]);
}

#[test]
fn comment() {
    assert_eq!(
        get_tokens(LexerMonad::new_unnamed("1 ; 2 \n\t  3 ")),
        vec![Token::Number(Number::Int(1)), Token::Number(Number::Int(3))]
    );
}

#[test]
fn string() {
    assert_eq!(
        get_tokens(LexerMonad::new_unnamed(
            "\"a b c\n d\" ; \" e f\" \n\t  \"\" "
        )),
        vec![Token::String("a b c\n d".into()), Token::String("".into())]
    );
}

#[test]
fn quote_token() {
    assert_eq!(
        get_tokens(LexerMonad::new_unnamed("'(1 2 3)")),
        vec![
            Token::Quote,
            Token::LParem,
            Token::Number(Number::Int(1)),
            Token::Number(Number::Int(2)),
            Token::Number(Number::Int(3)),
            Token::RParem
        ]
    );
}

#[test]
fn keyword_def() {
    assert_eq!(
        get_tokens(LexerMonad::new_unnamed("(def foo 42)")),
        vec![
            Token::LParem,
            Token::Symbol("def".into()),
            Token::Symbol("foo".into()),
            Token::Number(Number::Int(42)),
            Token::RParem
        ]
    );
}

#[test]
fn keyword_set() {
    assert_eq!(
        get_tokens(LexerMonad::new_unnamed("(set foo 42)")),
        vec![
            Token::LParem,
            Token::Symbol("set".into()),
            Token::Symbol("foo".into()),
            Token::Number(Number::Int(42)),
            Token::RParem
        ]
    );
}

#[test]
fn keyword_lambda() {
    assert_eq!(
        get_tokens(LexerMonad::new_unnamed("(lambda (x) (+ x 1))")),
        vec![
            Token::LParem,
            Token::Symbol("lambda".into()),
            Token::LParem,
            Token::Symbol("x".into()),
            Token::RParem,
            Token::LParem,
            Token::Symbol("+".into()),
            Token::Symbol("x".into()),
            Token::Number(Number::Int(1)),
            Token::RParem,
            Token::RParem
        ]
    );
}

#[test]
fn symbol_token() {
    assert_eq!(
        get_tokens(LexerMonad::new_unnamed("foo")),
        vec![Token::Symbol("foo".into())]
    );
}

#[test]
fn mixed_tokens() {
    assert_eq!(
        get_tokens(LexerMonad::new_unnamed("(lambda (x) (def foo x))")),
        vec![
            Token::LParem,
            Token::Symbol("lambda".into()),
            Token::LParem,
            Token::Symbol("x".into()),
            Token::RParem,
            Token::LParem,
            Token::Symbol("def".into()),
            Token::Symbol("foo".into()),
            Token::Symbol("x".into()),
            Token::RParem,
            Token::RParem
        ]
    );
}

#[test]
fn symbol_with_numbers() {
    assert_eq!(
        get_tokens(LexerMonad::new_unnamed("abc123")),
        vec![Token::Symbol("abc123".into())]
    );
}

#[test]
fn multiple_whitespace() {
    assert_eq!(
        get_tokens(LexerMonad::new_unnamed("(  1   2 )")),
        vec![
            Token::LParem,
            Token::Number(Number::Int(1)),
            Token::Number(Number::Int(2)),
            Token::RParem
        ]
    );
}

#[test]
fn dot() {
    assert_eq!(
        get_tokens(LexerMonad::new_unnamed("(a . b)")),
        vec![
            Token::LParem,
            Token::Symbol("a".into()),
            Token::Dot,
            Token::Symbol("b".into()),
            Token::RParem
        ]
    );
}

#[test]
fn unknown_characters() {
    assert_eq!(
        get_tokens(LexerMonad::new_unnamed("$")),
        vec![Token::Symbol("$".into())]
    );
}
