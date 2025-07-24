use relic::{
    RT,
    env::Env,
    lexer::Number,
    rt_current_env, rt_get_integer, rt_start,
    runtime::{Runtime, StackMachine},
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
    let mut runtime = Runtime::new(10);
    let nil = runtime.new_symbol(Symbol::Nil);
    let root = runtime.new_pair(nil, nil);
    runtime.set_root("root".to_string(), root);
    let p1 = runtime.new_pair(nil, nil);
    runtime.set_car(true, root, p1).unwrap();
    let p2 = runtime.new_pair(nil, nil);
    runtime.set_cdr(true, p1, p2).unwrap();
    runtime.set_cdr(true, root, p1).unwrap();
    // After 3 allocations: root, p1, p2, nil
    assert_eq!(runtime.get_free(), 4);
}

#[test]
fn gc_test_collect() {
    let mut runtime = Runtime::new(10);
    let nil = runtime.new_symbol(Symbol::Nil);
    let root1 = runtime.new_pair(nil, nil);
    let root2 = runtime.new_pair(nil, nil);
    runtime.set_root("root1".to_string(), root1);
    runtime.set_root("root2".to_string(), root2);
    assert_eq!(runtime.get_free(), 3); // nil + 2 roots

    // Set root1 = (1 . 2), root2 = (3 . 4)
    let n1 = runtime.new_number(Number::Int(1));
    let n2 = runtime.new_number(Number::Int(2));
    let root1_idx = runtime.get_root("root1");
    runtime.set_car(true, root1_idx, n1).unwrap();
    runtime.set_cdr(true, root1_idx, n2).unwrap();
    let n3 = runtime.new_number(Number::Int(3));
    let n4 = runtime.new_number(Number::Int(4));
    let root2_idx = runtime.get_root("root2");
    runtime.set_car(true, root2_idx, n3).unwrap();
    runtime.set_cdr(true, root2_idx, n4).unwrap();

    // Allocate some garbage
    let garbage1 = runtime.new_pair(root1_idx, root2_idx);
    let gcar = runtime.new_pair(root1_idx, root2_idx);
    let gcdr = runtime.new_pair(root1_idx, root2_idx);
    runtime.set_car(true, garbage1, gcar).unwrap();
    runtime.set_cdr(true, garbage1, gcdr).unwrap();
    // Should have filled up the area and triggered GC on next alloc
    let before_gc_free = runtime.get_free();
    println!("before: {runtime}");
    let _garbage2 = runtime.new_pair(root1_idx, root2_idx); // triggers GC
    let after_gc_free = runtime.get_free();
    println!("after: {runtime}");
    // GC should reclaim `garbage1`, `gcar`, `gcdr` and allocate `garbage2`
    assert_eq!(after_gc_free, before_gc_free - 3);

    // Data is preserved
    // root1 = (1 . 2)
    let root1_idx = runtime.get_root("root1");
    // root1 = (1 . 2)
    let car_op = runtime.new_constant("car").unwrap();
    let cdr_op = runtime.new_constant("cdr").unwrap();

    // Get car of root1
    runtime.push(root1_idx);
    runtime.push(root1_idx);
    runtime.push(car_op);
    runtime.apply(1).unwrap();
    let car1 = runtime.pop();
    let v1 = runtime.get_number(car1).unwrap();
    assert_eq!(v1, Number::Int(1));

    // Get cdr of root1
    runtime.push(root1_idx);
    runtime.push(root1_idx);
    runtime.push(cdr_op);
    runtime.apply(1).unwrap();
    let cdr1 = runtime.pop();
    let v2 = runtime.get_number(cdr1).unwrap();
    assert_eq!(v2, Number::Int(2));

    // root2 = (3 . 4)
    let root2_idx = runtime.get_root("root2");

    // Get car of root2
    runtime.push(root2_idx);
    runtime.push(root2_idx);
    runtime.push(car_op);
    runtime.apply(1).unwrap();
    let car2 = runtime.pop();
    let v3 = runtime.get_number(car2).unwrap();
    assert_eq!(v3, Number::Int(3));

    // Get cdr of root2
    runtime.push(root2_idx);
    runtime.push(root2_idx);
    runtime.push(cdr_op);
    runtime.apply(1).unwrap();
    let cdr2 = runtime.pop();
    let v4 = runtime.get_number(cdr2).unwrap();
    assert_eq!(v4, Number::Int(4));
}

#[test]
fn gc_test_linklst() {
    with_different_gc_size(1, 20, |runtime| {
        let nil = runtime.new_symbol(Symbol::Nil);

        let root = runtime.new_pair(nil, nil);
        runtime.set_root("root".to_string(), root);
        runtime.set_root("temp".to_string(), root);

        let car_op = runtime.new_constant("car").unwrap();
        runtime.set_root("car".to_string(), car_op);
        let cdr_op = runtime.new_constant("cdr").unwrap();
        runtime.set_root("cdr".to_string(), cdr_op);

        let length = 100;
        for i in 0..length {
            let n = runtime.new_number(Number::Int(i));
            let mut temp_idx = runtime.get_root("temp");
            runtime.set_car(true, temp_idx, n).unwrap();
            let nil = runtime.new_symbol(Symbol::Nil);
            let next = runtime.new_pair(nil, nil);
            temp_idx = runtime.get_root("temp");
            runtime.set_cdr(true, temp_idx, next).unwrap();
            runtime.set_root("temp".to_string(), next);
        }
        let mut cur = runtime.get_root("root");
        for i in 0..length {
            runtime.push(cur);
            runtime.push(cur);
            let car_op = runtime.get_root("car");
            runtime.push(car_op);
            runtime.apply(1).unwrap();
            let car = runtime.pop();
            let cdr_op = runtime.get_root("cdr");
            runtime.push(cdr_op);
            runtime.apply(1).unwrap();
            let cdr = runtime.pop();
            let v = runtime.get_number(car).unwrap();
            assert_eq!(v, Number::Int(i));
            cur = cdr;
        }
    })
}

#[test]
fn gc_test_loop() {
    with_different_gc_size(10, 20, |runtime| {
        let mut nil = runtime.new_symbol(Symbol::Nil);
        let root_pair = runtime.new_pair(nil, nil);
        runtime.set_root("root".to_string(), root_pair);
        runtime.set_root("temp".to_string(), nil);

        // Create a loop in garbage
        let loop_size = 3;
        let garbage = runtime.new_pair(nil, nil);
        let mut temp2 = garbage;
        for i in 0..loop_size {
            let n = runtime.new_number(Number::Int(i));
            runtime.set_car(true, temp2, n).unwrap();
            let next = runtime.new_pair(nil, nil);
            runtime.set_cdr(true, temp2, next).unwrap();
            temp2 = next;
        }
        runtime.set_cdr(true, temp2, garbage).unwrap();
        println!("finished generating garbage");

        let loop_size = 10;
        let root_idx = runtime.get_root("root");
        runtime.set_root("temp".to_string(), root_idx);
        for i in 0..loop_size {
            let n = runtime.new_number(Number::Int(i));
            runtime.set_car(true, runtime.get_root("temp"), n).unwrap();
            nil = runtime.new_symbol(Symbol::Nil);
            let next = runtime.new_pair(nil, nil);
            runtime
                .set_cdr(true, runtime.get_root("temp"), next)
                .unwrap();
            runtime.set_root("temp".to_string(), next);
        }
        let n = runtime.new_number(Number::Int(loop_size));
        let temp_idx = runtime.get_root("temp");
        let root_idx = runtime.get_root("root");
        runtime.set_car(true, temp_idx, n).unwrap();
        runtime.set_cdr(true, temp_idx, root_idx).unwrap();

        let mut cur = runtime.get_root("root");

        for _ in 0..2 {
            for i in 0..=loop_size {
                runtime.push(cur);
                runtime.push(cur);
                let car_op = runtime.new_constant("car").unwrap();
                runtime.push(car_op);
                runtime.apply(1).unwrap();
                let car = runtime.pop();
                let v = runtime.get_number(car).unwrap();
                assert_eq!(v, Number::Int(i));

                let cdr_op = runtime.new_constant("cdr").unwrap();
                runtime.push(cdr_op);
                runtime.apply(1).unwrap();
                let cdr = runtime.pop();

                cur = cdr;
            }
        }
        // Check that garbage is collected (free == (loop_size+1)*2)
        runtime.debug_force_gc();
        let used = runtime.get_free();
        assert!(used == ((loop_size + 1) * 2).try_into().unwrap());
    })
}

#[test]
fn parse_test() {
    with_different_gc_size(1, 20, |runtime| {
        let five = runtime.new_constant("5").unwrap();
        runtime.push(five);
        let twelve = runtime.new_constant("12").unwrap();
        runtime.push(twelve);
        let div = runtime.new_constant("/").unwrap();
        runtime.push(div);
        runtime.apply(2).unwrap();
        let y = runtime.pop();
        assert_eq!(runtime.display_node_idx(y), "2.4");
        let list = runtime.new_constant("(2 3)").unwrap();
        runtime.push(list);
        let y = runtime.pop();
        assert_eq!(runtime.display_node_idx(y), "(2 3)");
        let complex = runtime
            .new_constant(";asdf\n(2 (3 . /) ; comment\n <= (5 a) )")
            .unwrap();
        runtime.push(complex);
        let y = runtime.pop();
        assert_eq!(runtime.display_node_idx(y), "(2 (3 . /) <= (5 a))");
    })
}

#[test]
fn arith_test() {
    let mut runtime = Runtime::new(5);
    let x = runtime.new_number(Number::Int(1));
    let y = runtime.new_number(Number::Int(2));
    runtime.set_root("x".to_string(), x);
    runtime.set_root("y".to_string(), y);
    runtime.push(x);
    runtime.push(y);
    let num = runtime.new_constant("3.1").unwrap();
    runtime.push(num);
    let plus = runtime.new_constant("+").unwrap();
    runtime.push(plus);
    runtime.apply(3).unwrap();
    let ret = runtime.pop();
    let val = runtime.get_number(ret).unwrap();
    assert_eq!(val, Number::Float(6.1));

    let one = runtime.new_constant("1").unwrap();
    runtime.push(one);
    let x1 = runtime.new_number(Number::Int(4));
    runtime.push(x1);
    let x2 = runtime.new_number(Number::Int(5));
    runtime.push(x2);
    let minus = runtime.new_constant("-").unwrap();
    runtime.push(minus);
    runtime.apply(3).unwrap();

    let ret = runtime.pop();
    let val = runtime.get_number(ret).unwrap();
    assert_eq!(val, Number::Int(0));
}

#[test]
fn rel_test() {
    let mut runtime = Runtime::new(5);
    let x = runtime.new_number(Number::Int(1));
    let y = runtime.new_number(Number::Int(2));
    runtime.set_root("x".to_string(), x);
    runtime.set_root("y".to_string(), y);
    runtime.push(x);
    runtime.push(y);
    let lt = runtime.new_constant("<").unwrap();
    runtime.push(lt);
    runtime.apply(2).unwrap();
    let ret = runtime.pop();
    let val = runtime.get_symbol(ret).unwrap();
    assert_eq!(val, Symbol::Nil);
    let y = runtime.get_root("y");
    runtime.push(y);
    let two = runtime.new_constant("2").unwrap();
    runtime.push(two);
    let eq = runtime.new_constant("=").unwrap();
    runtime.push(eq);
    runtime.apply(2).unwrap();
    let ret = runtime.pop();
    let val = runtime.get_symbol(ret).unwrap();
    assert_eq!(val, Symbol::T);
}

#[test]
fn static_test() {
    rt_start();
    let depth = 500;
    {
        let mut runtime = RT.lock().unwrap();
        let expr = runtime.new_constant("(/ (3.1 4) * (+ . -))").unwrap();
        runtime.push(expr);
        let cdr = runtime.new_constant("cdr").unwrap();
        runtime.push(cdr);
        runtime.apply(1).unwrap();
        let car = runtime.new_constant("car").unwrap();
        runtime.push(car);
        runtime.apply(1).unwrap();
        let car2 = runtime.new_constant("car").unwrap();
        runtime.push(car2);
        runtime.apply(1).unwrap();
        let x = runtime.pop();
        let x = runtime.get_number(x).unwrap();
        assert_eq!(x, Number::Float(3.1));
        let mut outer = runtime.current_env();
        for i in 0..depth {
            let mut env = runtime.new_env(format!("env {i}"), outer);
            runtime.move_to_env(env);
            let num = runtime.new_number(Number::Int(i));
            env = runtime.current_env();
            runtime.insert_cur_env(env, &"i".to_string(), num);
            outer = runtime.current_env();
        }
    }
    let env = rt_current_env();
    let x = env.get(&"i".to_string(), &mut RT.lock().unwrap()).unwrap();
    let x = rt_get_integer(x);
    assert_eq!(x, depth - 1)
}
