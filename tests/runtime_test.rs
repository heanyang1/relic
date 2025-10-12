use std::collections::HashMap;

use relic::{
    number::Number,
    runtime::{LoadToRuntime, Runtime, RuntimeNode, StackMachine},
    symbol::Symbol,
};

fn with_different_gc_size<T>(lb: usize, ub: usize, test: T)
where
    T: Fn(&mut Runtime),
{
    for i in lb..ub {
        let mut runtime = Runtime::new(i);
        test(&mut runtime);
    }
}

#[test]
fn gc_test_simple() {
    let mut runtime = Runtime::new(100);
    Symbol::Nil.load_to(&mut runtime).unwrap();
    Symbol::Nil.load_to(&mut runtime).unwrap();
    runtime.new_pair();
    let root = runtime.pop();
    runtime.add_root("root".to_string(), root);

    Symbol::Nil.load_to(&mut runtime).unwrap();
    Symbol::Nil.load_to(&mut runtime).unwrap();
    runtime.new_pair();
    let p1 = runtime.pop();
    runtime.add_root("p1".to_string(), p1);

    Symbol::Nil.load_to(&mut runtime).unwrap();
    Symbol::Nil.load_to(&mut runtime).unwrap();
    runtime.new_pair();
    let p2 = runtime.pop();

    let root = runtime.get_root("root");
    let p1 = runtime.remove_root("p1");
    runtime.set_car(true, root, p1).unwrap();
    runtime.set_cdr(true, p1, p2).unwrap();
    runtime.set_cdr(true, root, p2).unwrap();

    runtime.gc();
    // root, p1, p2, nil * 3
    assert_eq!(runtime.get_free(), 6);
}

#[test]
fn gc_test_linklst() {
    with_different_gc_size(1, 20, |runtime| {
        let mut list_str = "(".to_string();
        let length = 20;
        for i in 0..length {
            list_str += &format!("{i} ");
        }
        list_str += ")";
        list_str.load_to(runtime).unwrap();
        let mut cur = runtime.pop();
        for i in 0..length {
            let num = runtime.get_node(true, cur);
            if let RuntimeNode::Pair(car, cdr) = num {
                let car = runtime.get_number(*car).unwrap();
                assert_eq!(car, Number::Int(i));
                cur = *cdr;
            } else {
                panic!("not a pair");
            }
        }
    })
}

#[test]
fn parse_test() {
    with_different_gc_size(1, 20, |runtime| {
        "5".load_to(runtime).unwrap();
        "12".load_to(runtime).unwrap();
        "2".load_to(runtime).unwrap();
        "/".load_to(runtime).unwrap();
        runtime.apply().unwrap();
        let y = runtime.pop();
        assert_eq!(runtime.display_node_idx(y), "2.4");
        "(2 3)".load_to(runtime).unwrap();
        let y = runtime.pop();
        assert_eq!(runtime.display_node_idx(y), "(2 3)");
        ";asdf\n(2 (3 . /) ; comment\n <= (5 a) )"
            .load_to(runtime)
            .unwrap();
        let y = runtime.pop();
        assert_eq!(runtime.display_node_idx(y), "(2 (3 . /) <= (5 a))");
    })
}

#[test]
fn cycle_test() {
    with_different_gc_size(1, 20, |runtime| {
        let mut list_str = "(".to_string();
        let mut loop_str = "(".to_string();
        let length = 50;
        for i in 0..length {
            list_str += &format!("{i} ");
            loop_str += &format!("{i} ");
        }
        list_str += ")";
        loop_str += ". #0#)";
        list_str.load_to(runtime).unwrap();

        let first = runtime.pop();
        runtime.add_root("first".to_string(), first);
        runtime.push(first);

        for _ in 0..length - 1 {
            let cur = runtime.pop();
            let (_, cdr) = runtime.get_pair(cur).unwrap();
            runtime.push(cdr);
        }

        let last = runtime.pop();
        let first = runtime.get_root("first");
        runtime.set_cdr(true, last, first).unwrap();
        runtime.remove_root("first");

        let node = runtime.to_node(first, &mut HashMap::new());
        assert_eq!(loop_str, format!("{}", node.borrow()));

        // node.load_to(runtime).unwrap();
        // runtime.gc();

        // let node = runtime.pop();
        // assert_eq!(loop_str, format!("{}", runtime.display_node_idx(node)));
    })
}
