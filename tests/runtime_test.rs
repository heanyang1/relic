use relic::{
    RT,
    compile::{CodeGen, Compile},
    lexer::{Lexer, Number},
    node::Node,
    parser::Parse,
    preprocess::PreProcess,
    rt_get, rt_import, rt_start,
    runtime::{LoadToRuntime, Runtime, RuntimeNode, StackMachine},
    symbol::Symbol,
};
use std::{collections::HashMap, ffi::CString, process::Command};

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
        "/".load_to(runtime).unwrap();
        runtime.apply(2).unwrap();
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

macro_rules! compile {
    ($input:expr, $package_name:expr) => {{
        let mut tokens = Lexer::new($input);
        let mut macros = HashMap::new();
        let mut codegen = CodeGen::new_library($package_name);

        while let Ok(mut node) = Node::parse(&mut tokens) {
            let node = node.preprocess(&mut macros).unwrap();
            node.compile(&mut codegen).unwrap();
        }
        codegen
    }};
}

#[test]
fn compile_test() {
    let code = "(define x (+ 1 2))";
    let lib_name = "mylib";
    let codegen = compile!(code, lib_name.to_string());
    let c_code = codegen.to_string();
    std::fs::write(format!("/tmp/relic_{lib_name}.c"), c_code).unwrap();

    let status = Command::new("gcc")
        .args([
            "-Ic_runtime",
            "-shared",
            "-fPIC",
            "-o",
            &format!("./lib/{lib_name}.relic"),
            &format!("/tmp/relic_{lib_name}.c"),
        ])
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
    assert!(status.success());

    rt_start();
    let c_str = CString::new(lib_name).unwrap();
    rt_import(c_str.as_bytes().as_ptr());

    let c_str = CString::new("x").unwrap();
    let x = rt_get(c_str.as_bytes().as_ptr());
    let runtime = RT.lock().unwrap();
    println!("x={}", runtime.display_node_idx(x));
}
