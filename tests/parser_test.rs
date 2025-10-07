#[cfg(test)]
mod tests {
    use relic::lexer::{Lexer, Number};
    use relic::node::Node;
    use relic::parser::Parse;
    use relic::symbol::{SpecialForm, Symbol};
    use relic::{nil, vec_to_list};

    #[test]
    fn test_parse_number() {
        let input = "42";
        let mut lexer = Lexer::new(input);
        let result = Node::parse(&mut lexer);
        assert_eq!(result, Ok(Node::Number(Number::Int(42))));
        let input = "3.14159265358979323846";
        let mut lexer = Lexer::new(input);
        let result = Node::parse(&mut lexer);
        assert_eq!(result, Ok(Node::Number(Number::Float(3.14159265358979323846))));
    }

    #[test]
    fn test_parse_symbol() {
        let input = "x";
        let mut lexer = Lexer::new(input);
        let result = Node::parse(&mut lexer);
        assert_eq!(result, Ok(Node::Symbol(Symbol::User("x".to_string()))));
    }

    #[test]
    fn test_parse_add() {
        let input = "+";
        let mut lexer = Lexer::new(input);
        let result = Node::parse(&mut lexer);
        assert_eq!(result, Ok(Node::Symbol(Symbol::Add)));
    }

    #[test]
    fn test_parse_sexp() {
        let input = "(+ 1 2)";
        let mut lexer = Lexer::new(input);
        let result = Node::parse(&mut lexer);

        assert_eq!(
            result,
            Ok(Node::Pair(
                Node::Symbol(Symbol::Add).into(),
                Node::Pair(
                    Node::Number(Number::Int(1)).into(),
                    Node::Pair(Node::Number(Number::Int(2)).into(), nil!().into()).into()
                )
                .into()
            ))
        );
    }

    #[test]
    fn test_nested_expressions() {
        let input = "(+ (* 2 3) (- 5 1))";
        let mut lexer = Lexer::new(input);
        let result = Node::parse(&mut lexer);

        let ret = vec_to_list!(
            Node::Symbol(Symbol::Add).into(),
            vec_to_list!(
                Node::Symbol(Symbol::Mul).into(),
                Node::Number(Number::Int(2)).into(),
                Node::Number(Number::Int(3)).into()
            )
            .into(),
            vec_to_list!(
                Node::Symbol(Symbol::Sub).into(),
                Node::Number(Number::Int(5)).into(),
                Node::Number(Number::Int(1)).into()
            )
            .into()
        );
        assert_eq!(result, Ok(ret));
    }

    #[test]
    fn test_pair() {
        let input = "(1 . 2)";
        let mut lexer = Lexer::new(input);
        let result = Node::parse(&mut lexer);

        assert_eq!(
            result,
            Ok(Node::Pair(
                Node::Number(Number::Int(1)).into(),
                Node::Number(Number::Int(2)).into()
            ))
        );
    }

    #[test]
    fn test_empty_sexp() {
        let input = "()";
        let mut lexer = Lexer::new(input);
        let result = Node::parse(&mut lexer);

        assert_eq!(result, Ok(nil!()));
    }

    #[test]
    fn test_comment() {
        let input = "(;\n);;";
        let mut lexer = Lexer::new(input);
        let result = Node::parse(&mut lexer);

        assert_eq!(result, Ok(nil!()));
    }

    #[test]
    fn test_quote() {
        let input = "'(() '())";
        let mut lexer = Lexer::new(input);
        let result = Node::parse(&mut lexer);

        // (quote (() (quote ())))
        let ret = vec_to_list!(
            Node::SpecialForm(SpecialForm::Quote).into(),
            vec_to_list!(
                nil!().into(),
                vec_to_list!(Node::SpecialForm(SpecialForm::Quote).into(), nil!().into()).into()
            )
            .into()
        );
        assert_eq!(result, Ok(ret));
    }

    #[test]
    fn test_invalid_statement() {
        // "x)" is valid for one statement (which will be parsed as a symbol "x"),
        // but it is not a valid program
        let inputs = [
            "(",
            ")",
            "(def x",
            "(((()(())())",
            "(1 2 .)",
            "(. 1)",
            "(1 . 2 3)",
            ".",
        ];

        for input in &inputs {
            let mut lexer = Lexer::new(input);
            let result = Node::parse(&mut lexer);
            assert!(result.is_err());
        }
    }

    // #[test]
    // fn test_display() {
    //     let input = "((* 1 (+ 2 3)) (4 5 . 6) (car (((7) 8)) cdr))";
    //     let mut lexer = Lexer::new(input);
    //     let result = Node::parse(&mut lexer);
    //     let node: Rc<RefCell<Node>> = result.unwrap().into();
    //     let mut runtime = Runtime::new(1);
    //     node.load_to(&mut runtime).unwrap();
    //     let node = runtime.pop();
    //     assert_eq!(format!("{}", runtime.display_node_idx(node)), input);
    // }
}
