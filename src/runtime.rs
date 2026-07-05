//! The runtime module.
//!
//! This module implements the core runtime for the Relic Lisp interpreter. It provides
//! a stack-based virtual machine with garbage collection, environment management, and
//! debugger support.
//!
//! ## Architecture Overview
//!
//! The runtime is designed as a stack machine that evaluates Lisp expressions. Unlike
//! traditional register-based virtual machines (like SICP's register machine), Relic
//! uses a stack for simplicity and to avoid parsing assignment expressions.
//!
//! ## Stack Machine
//!
//! The [`StackMachine`] trait defines the core operations:
//! - **Push**: Push an item (index to GC area) onto the stack
//! - **Pop**: Pop an item from the stack
//! - **Top**: View the top item without popping
//! - **Swap**: Swap the top two elements
//! - **Apply**: Pop operator and operands, evaluate, push result
//!
//! The stack contains indices into the GC area, not the actual data. This design
//! simplifies ownership and allows the garbage collector to manage all heap data.
//!
//! ### Evaluation Model
//!
//! To evaluate an expression like `(+ 1 2)`:
//! 1. Push the arguments: push `1`, push `2`
//! 2. Push the number of arguments: push `2`
//! 3. Push the operator: push `+`
//! 4. Call `apply()` which pops the operator, pops nargs, pops nargs operands,
//!    evaluates the operation, and pushes the result
//!
//! ## Garbage Collection
//!
//! Relic's GC is completely modeled after SICP's GC. The GC area is split into
//! two halves ("areas"):
//!
//! - **From space**: The active area where new objects are allocated
//! - **To space**: The inactive area where objects are copied during GC
//!
//! ### GC Algorithm
//!
//! 1. **Allocation**: New objects are simply appended to the from-space vector.
//!    If from-space is full, GC is triggered.
//!
//! 2. **Collection Phase**:
//!    - Start from root variables and stack elements
//!    - Use depth-first search (via [`gc_dfs`]) to copy all reachable nodes
//!    - For each reachable node:
//!      - Copy it to to-space
//!      - Leave a "broken heart" marker in from-space pointing to new location
//!      - Recursively process children (pairs, environments, closures)
//!
//! 3. **Compaction**: After collection, swap the two areas. If no memory was
//!    reclaimed (same free count), double the area size.
//!
//! ### Broken Heart Technique
//!
//! When an object is copied from from-space to to-space, the original location
//! is replaced with a `BrokenHeart` variant containing the new index. If another
//! object points to this relocated object, `gc_dfs` detects the broken heart
//! and returns the new location directly without recursing.
//!
//! ## RuntimeNode Types
//!
//! The runtime can store these types:
//! - [`RuntimeNode::Symbol`]: Lisp symbols (nil, t, built-ins, user-defined)
//! - [`RuntimeNode::Number`]: Integer and floating-point numbers
//! - [`RuntimeNode::Pair`]: Cons cells (car, cdr as indices)
//! - [`RuntimeNode::Environment`]: Variable bindings with lexical scoping
//! - [`RuntimeNode::Closure`]: Function objects with captured environment
//! - [`RuntimeNode::BrokenHeart`]: Forwarding pointer during GC
//!
//! ## Environments
//!
//! Environments implement lexical scoping via a chain:
//! - Each environment has a map of variable name -> value index
//! - Each environment optionally references an outer environment
//! - Variable lookup searches current env, then walks the chain outward
//! - `set!` modifies the first environment in the chain that contains the variable
//!
//! ## Closures
//!
//! Closures capture the environment at creation time. When a lambda is evaluated:
//! 1. A closure object is created with the current environment reference
//! 2. When called, a new environment is created with the closure's env as outer
//! 3. Arguments are bound in this new environment
//!
//! ## FFI Considerations
//!
//! All data access is through indices to avoid ownership issues in C bindings.
//! The FFI functions in `lib.rs` demonstrate this pattern: they take indices
//! as parameters and return indices or primitive types.

use std::{collections::HashMap, fmt::Display, mem::swap, result::Result, vec::Vec};

use crate::{
    env::Env,
    error::{ParseError, RuntimeError},
    lexer::LexerMonad,
    logger::log_error,
    node::{Node, PrintableNode},
    number::Number,
    symbol::Symbol,
    util::{CVoidFunc, eval_arith, eval_rel, map_to_assoc_lst},
};

use libloading::Library;

/// A handle to a loaded package, keeping the underlying shared library or JIT
/// code alive.
pub enum PackageHandle {
    Library(Library),
    Jit,
}

/// Closures.
///
/// This is probably the easiest way to represent lambdas using C function.
/// See [this blog post](https://matt.might.net/articles/closure-conversion/)
/// for details.
///
/// Our closure is even simpler than that in the blog post. The function accepts
/// no argument and extracts its arguments from current environment and stack.
/// It pushes its return value to the stack when finished.
#[derive(Debug, Clone, Eq)]
pub struct Closure {
    /// Closure name. It should be unique.
    ///
    /// Currently it is only used in renaming variables in closures: The n-th
    /// variable of the closure `xyz` will have name `#n_func_xyz`.
    pub(crate) name: String,
    /// The function body.
    pub(crate) body: CVoidFunc,
    /// The environment where the closure is constructed.
    pub(crate) env: usize,
    /// Number of arguments.
    pub(crate) nargs: usize,
    /// Whether the closure is variadic. If it is `true`, then the last argument
    /// of the closure is the list of remaining arguments.
    pub(crate) variadic: bool,
}

impl PartialEq for Closure {
    fn eq(&self, _: &Self) -> bool {
        panic!("Comparing closures")
    }
}

impl Closure {
    pub fn new(
        name: String,
        body: CVoidFunc,
        nargs: usize,
        variadic: bool,
        runtime: &Runtime,
    ) -> Closure {
        Closure {
            name,
            body,
            env: runtime.current_env(),
            nargs,
            variadic,
        }
    }
}

// Environment manipulation.
impl Env<String, usize, Runtime> for usize {
    fn get_cur(&self, key: &String, runtime: &Runtime) -> Option<usize> {
        runtime.get_cur_env(*self, key)
    }
    fn do_in_outer<Out, F>(&self, func: F, runtime: &Runtime) -> Out
    where
        F: Fn(&Self) -> Out,
        Self: Sized,
    {
        // `outer = ...` and `func(...)` acquire and release the lock respectively
        // so they must be separated into two statements.
        let outer = runtime.get_outer_env(*self);
        func(&outer.unwrap())
    }
    fn do_in_outer_mut<Out, F>(&mut self, func: F, runtime: &mut Runtime) -> Out
    where
        F: Fn(&mut Self, &mut Runtime) -> Out,
        Self: Sized,
    {
        let outer = runtime.get_outer_env(*self);
        func(&mut outer.unwrap(), runtime)
    }
    fn has_outer(&self, runtime: &Runtime) -> bool {
        runtime.get_outer_env(*self).is_some()
    }
    fn insert_cur(&mut self, key: &String, value: usize, runtime: &mut Runtime) {
        runtime.insert_cur_env(*self, key, value);
    }
}

/// The runtime data node. A runtime data node is owned by the garbage
/// collector and is used by the user to store data structures at run-time.
#[derive(Debug, Clone)]
pub enum RuntimeNode {
    /// Symbols.
    Symbol(Symbol),
    /// Numbers.
    Number(Number),
    /// Pair of nodes.
    Pair(usize, usize),
    /// Environments.
    /// Fields are: (Name, Variable map, Outer environment)
    Environment(String, HashMap<String, usize>, Option<usize>),
    /// Closures.
    Closure(Closure),
    /// Indicates the data is moved to the [data field] position of the other area.
    BrokenHeart(usize),
}

/// The runtime.
///
/// To simplify bindings and avoid ownership issues, users can only get the
/// index of the runtime node in the GC area. There are functions that retrives
/// the content of the node through index.
pub struct Runtime {
    /// The stack. Its content is the index to the element in the GC area.
    ///
    /// The stack element won't be GCed.
    stack: Vec<usize>,
    /// The GC area is split into two halves.
    /// The first one is always the one being used.
    areas: (Vec<RuntimeNode>, Vec<RuntimeNode>),
    /// Size of the GC area in pairs.                            
    size: usize,
    /// Root variables that won't be GCed.
    ///
    /// The key is its name and the value is its index.
    roots: HashMap<String, usize>,
    /// Opened packages.
    ///
    /// This field is not used, but we need to keep it so that we can use the
    /// C function pointers inside the shared library.
    packages: HashMap<String, PackageHandle>,
}

impl Display for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "area: [")?;
        for node in 0..self.areas.0.len() {
            write!(f, "{} ", self.display_node_idx(node))?;
        }
        writeln!(f, "]")?;
        write!(f, "stack: [")?;
        for node in self.stack.clone() {
            write!(f, "{} ", self.display_node_idx(node))?;
        }
        writeln!(f, "]")?;
        writeln!(f, "roots: [")?;
        for (name, node) in self.roots.clone() {
            writeln!(f, "\t{}: {}", name, self.display_node_idx(node))?;
        }
        writeln!(f, "]")?;
        Ok(())
    }
}

/// The trait that describes how to move an object into the runtime.
pub trait LoadToRuntime {
    /// Load the object into the top of the stack.
    ///
    /// # Errors
    ///
    /// Returns [ParseError] and restores the stack to the state before the
    /// function call if an error occurs.
    fn load_to(self, runtime: &mut Runtime) -> Result<(), ParseError>;
}

impl LoadToRuntime for Number {
    fn load_to(self, runtime: &mut Runtime) -> Result<(), ParseError> {
        RuntimeNode::Number(self).load_to(runtime)
    }
}

impl LoadToRuntime for Symbol {
    fn load_to(self, runtime: &mut Runtime) -> Result<(), ParseError> {
        RuntimeNode::Symbol(self).load_to(runtime)
    }
}

impl LoadToRuntime for Closure {
    fn load_to(self, runtime: &mut Runtime) -> Result<(), ParseError> {
        RuntimeNode::Closure(self).load_to(runtime)
    }
}

impl LoadToRuntime for String {
    fn load_to(self, runtime: &mut Runtime) -> Result<(), ParseError> {
        LexerMonad::new_unnamed(self).load_to(runtime)
    }
}

impl LoadToRuntime for LexerMonad<()> {
    fn load_to(self, runtime: &mut Runtime) -> Result<(), ParseError> {
        self.parse()?.load_to(runtime)
    }
}

impl LoadToRuntime for &LexerMonad<Node> {
    fn load_to(self, runtime: &mut Runtime) -> Result<(), ParseError> {
        match self.get() {
            Node::Number(num) => num.clone().load_to(runtime),
            Node::Pair(car, cdr) => {
                cdr.borrow().load_to(runtime)?;
                if let Err(e) = car.borrow().load_to(runtime) {
                    runtime.pop();
                    Err(e)
                } else {
                    runtime.new_pair();
                    Ok(())
                }
            }
            Node::SpecialForm(form) => {
                // Special forms are loaded as user symbol.
                Symbol::User(form.to_string()).load_to(runtime)
            }
            Node::String(s) => Symbol::User(s.to_string()).load_to(runtime),
            Node::Symbol(s) => s.clone().load_to(runtime),
        }
    }
}

impl LoadToRuntime for RuntimeNode {
    fn load_to(self, runtime: &mut Runtime) -> Result<(), ParseError> {
        let idx = runtime.new_node_with_gc(self);
        runtime.push(idx);
        Ok(())
    }
}

impl TryFrom<RuntimeNode> for Number {
    type Error = RuntimeError;
    fn try_from(value: RuntimeNode) -> Result<Self, Self::Error> {
        if let RuntimeNode::Number(number) = value {
            Ok(number.clone())
        } else {
            Err(RuntimeError::new(format!("{value:?} is not a number")))
        }
    }
}

macro_rules! rel_op {
    ($runtime:expr, $nargs:expr, $op:tt) => {{
        let operands = $runtime.node_vec_from_stack($nargs);
        load_to!($runtime, eval_rel(operands, |a, b| a $op b)?)
    }};
}

macro_rules! arith_op {
    ($runtime:expr, $nargs:expr, $op:tt) => {{
        let operands = $runtime.node_vec_from_stack($nargs);
        load_to!($runtime, eval_arith(operands, |a, b| a $op b)?)
    }};
}

macro_rules! unary_op {
    ($runtime:expr, $nargs:expr, $op:expr) => {{
        assert_eq!($nargs, 1);
        let val = $runtime.pop();
        match $runtime.get_number(val) {
            Ok(num) => $op(num)
                .load_to($runtime)
                .map_err(|e| RuntimeError::new(e.to_string())),
            Err(e) => Err(e),
        }
    }};
}

/// Unlike SICP's register machine model, our runtime uses a stack machine
/// to evaluate expression.
///
/// The `Item` can be either an operator or operand. We use the index of the
/// runtime node as `Item` in the runtime.
///
/// Here are some reasons to use stack machines (none of which will happen in
/// SICP's register machine, which is built on the top of a Scheme system):
/// - The problem of parsing assignment expressions is avoided.
///   `(assign a ((op +) (const 2) (reg x)))` can be done by pushing three items
///   and call `apply`, or by parsing the expression using a customized parser,
///   then walk down the AST. GC may be triggered during the process and the node
///   of `(const 2)` may be lost if you are not careful.
/// - In our implementation, stack operations are not slower than register
///   operation. The registers are implemented by a hash map and the stack is a
///   vector, both of them takes (amortized) O(1) time to insert and delete.
pub trait StackMachine<Item> {
    /// Whether the stack is empty.
    fn empty(&self) -> bool;
    /// Push an item to the stack.
    fn push(&mut self, item: Item);
    /// Pop an item from the stack. Panics when stack underflow.
    fn pop(&mut self) -> Item;
    /// Get the top item from the stack. Doesn't pop the item. Panics when
    /// stack underflow.
    fn top(&self) -> Item;
    /// Swap the top two elements in the stack. Panics when stack underflow.
    fn swap(&mut self);
    /// Pop an item as operator, an item as number of operands and items as
    /// operands, evaluate the expression, then push the result into the stack.
    fn apply(&mut self) -> Result<(), RuntimeError>;
}

impl StackMachine<usize> for Runtime {
    fn empty(&self) -> bool {
        self.stack.is_empty()
    }
    fn push(&mut self, index: usize) {
        self.stack.push(index);
    }
    fn pop(&mut self) -> usize {
        self.stack.pop().expect("Stack underflow")
    }
    fn top(&self) -> usize {
        *self.stack.iter().last().expect("Stack underflow")
    }
    fn swap(&mut self) {
        let len = self.stack.len();
        assert!(len >= 2, "Stack underflow");
        let (left, right) = self.stack.split_at_mut(len - 1);
        swap(&mut left[len - 2], &mut right[0]);
    }

    fn apply(&mut self) -> Result<(), RuntimeError> {
        macro_rules! load_to {
            ($runtime:expr, $expr:expr) => {
                $expr
                    .load_to($runtime)
                    .map_err(|e| RuntimeError::new(e.to_string()))
            };
        }

        let index = self.pop();
        let operator = self.get_symbol(index)?;
        let index = self.pop();
        let nargs = usize::try_from(self.get_number(index)?)?;

        match operator {
            Symbol::Nil | Symbol::T => Err(RuntimeError::new(format!(
                "{self} can not be the head of a list"
            ))),
            Symbol::Add => arith_op!(self, nargs, +),
            Symbol::Mul => arith_op!(self, nargs, *),
            Symbol::Sub => arith_op!(self, nargs, -),
            Symbol::Div => arith_op!(self, nargs, /),
            Symbol::Remainder => {
                assert_eq!(nargs, 2);
                let lhs = self.pop();
                let rhs = self.pop();
                if let (Ok(Number::Int(lhs)), Ok(Number::Int(rhs))) =
                    (self.get_number(lhs), self.get_number(rhs))
                {
                    load_to!(self, Number::Int(lhs % rhs))
                } else {
                    Err(RuntimeError::new(format!(
                        "Expected two integers, found {} and {}",
                        self.display_node_idx(lhs),
                        self.display_node_idx(rhs)
                    )))
                }
            }
            Symbol::Quotient => {
                assert_eq!(nargs, 2);
                let lhs = self.pop();
                let rhs = self.pop();
                if let (Ok(Number::Int(lhs)), Ok(Number::Int(rhs))) =
                    (self.get_number(lhs), self.get_number(rhs))
                {
                    load_to!(self, Number::Int(lhs / rhs))
                } else {
                    Err(RuntimeError::new(format!(
                        "Expected two integers, found {} and {}",
                        self.display_node_idx(lhs),
                        self.display_node_idx(rhs)
                    )))
                }
            }
            Symbol::Floor => {
                unary_op!(self, nargs, |num| {
                    Number::Int(f64::from(num).floor() as i64)
                })
            }
            Symbol::Ceiling => {
                unary_op!(self, nargs, |num| {
                    Number::Int(f64::from(num).ceil() as i64)
                })
            }
            Symbol::Sin => {
                unary_op!(self, nargs, |num| { Number::Float(f64::from(num).sin()) })
            }
            Symbol::Abs => {
                unary_op!(self, nargs, |num| {
                    match num {
                        Number::Int(num) => Number::Int(num.abs()),
                        Number::Float(num) => Number::Float(num.abs()),
                    }
                })
            }
            Symbol::Cos => {
                unary_op!(self, nargs, |num| { Number::Float(f64::from(num).cos()) })
            }
            Symbol::Eq => {
                assert_eq!(nargs, 2);
                let lhs = self.pop();
                let rhs = self.pop();
                load_to!(
                    self,
                    if self.node_eq(lhs, rhs) {
                        Symbol::T
                    } else {
                        Symbol::Nil
                    }
                )
            }
            Symbol::EqNum => rel_op!(self, nargs, ==),
            Symbol::Gt => rel_op!(self, nargs, >),
            Symbol::Lt => rel_op!(self, nargs, <),
            Symbol::Ge => rel_op!(self, nargs, >=),
            Symbol::Le => rel_op!(self, nargs, <=),
            Symbol::Atom => {
                assert_eq!(nargs, 1);
                let val = self.pop_as_node();
                load_to!(
                    self,
                    if let RuntimeNode::Pair(_, _) = val {
                        Symbol::Nil
                    } else {
                        Symbol::T
                    }
                )
            }
            Symbol::Car => {
                assert_eq!(nargs, 1);
                let index = self.top();
                let node_str = self.display_node_idx(index);
                let val = self.pop_as_node();
                if let RuntimeNode::Pair(car, _) = val {
                    self.push(car);
                    Ok(())
                } else {
                    Err(RuntimeError::new(format!("{node_str} is not a pair")))
                }
            }
            Symbol::Cdr => {
                assert_eq!(nargs, 1);
                let index = self.top();
                let node_str = self.display_node_idx(index);
                let val = self.pop_as_node();
                if let RuntimeNode::Pair(_, cdr) = val {
                    self.push(cdr);
                    Ok(())
                } else {
                    Err(RuntimeError::new(format!("{node_str} is not a pair")))
                }
            }
            Symbol::Cons => {
                self.new_pair();
                Ok(())
            }
            Symbol::List => {
                self.zip_stack_nodes(nargs);
                Ok(())
            }
            Symbol::Number => {
                assert_eq!(nargs, 1);
                load_to!(
                    self,
                    if let RuntimeNode::Number(_) = self.pop_as_node() {
                        Symbol::T
                    } else {
                        Symbol::Nil
                    }
                )
            }
            Symbol::User(_) => panic!("You should call the closure's function in C"),
        }
    }
}

// Package manipulation
impl Runtime {
    pub fn add_package(&mut self, name: String, package: PackageHandle) {
        assert!(self.packages.insert(name, package).is_none());
    }

    pub fn has_package(&self, name: &str) -> bool {
        self.packages.contains_key(name)
    }

    pub fn get_package(&self, name: &str) -> &PackageHandle {
        self.packages.get(name).unwrap()
    }
}

// Node creation and GC
impl Runtime {
    // GC and maintain the fields of `gc_area`.
    pub fn gc(&mut self) {
        let old_free = self.get_free();
        self.areas.1.clear();

        // Move all roots elements.
        for (name, root) in map_to_assoc_lst(&self.roots) {
            let new_root = self.gc_dfs(root);
            self.set_root(name, new_root);
        }
        // Move all stack elements.
        let new_stack = self.stack.clone();
        self.stack.clear();
        for elem in new_stack {
            let new_elem = self.gc_dfs(elem);
            self.stack.push(new_elem);
        }

        swap::<Vec<RuntimeNode>>(self.areas.0.as_mut(), self.areas.1.as_mut());
        if self.get_free() == old_free {
            // GC doesn't reclaim any memory. Increase the area size.
            self.size *= 2;
        }
    }
    // Try to call `gc()`.
    // Doesn't perform GC if there's enough memory to alloc a pair.
    pub fn try_gc(&mut self) {
        let old_free = self.get_free();
        if old_free < self.size {
            return;
        }
        self.gc();
    }
    fn gc_dfs(&mut self, cur: usize) -> usize {
        let node = self.get_node(true, cur);
        if let RuntimeNode::BrokenHeart(dst) = node {
            return *dst;
        }

        let dst_length = self.get_area(false).len();
        {
            // Allocate the space for the new item and invalidate the old item
            // to avoid calling `gc_dfs` on the same item again
            let node = node.clone();
            let dst_area = self.get_area_mut(false);
            dst_area.push(node);
            let src_area = self.get_area_mut(true);
            src_area[cur] = RuntimeNode::BrokenHeart(dst_length);
        }

        let content = match self.get_node(false, dst_length) {
            RuntimeNode::BrokenHeart(_) => panic!("Already moved"),
            RuntimeNode::Closure(Closure {
                name: id,
                body,
                env,
                nargs,
                variadic,
            }) => {
                let id = id.to_string();
                let body = *body;
                let nargs = *nargs;
                let variadic = *variadic;
                let env = self.gc_dfs(*env);
                RuntimeNode::Closure(Closure {
                    name: id,
                    body,
                    env,
                    nargs,
                    variadic,
                })
            }
            RuntimeNode::Environment(env_name, map, outer) => {
                let outer_clone = *outer;
                let env_name_clone = env_name.clone();
                let mut new_map = HashMap::new();
                for (name, var) in map_to_assoc_lst(map) {
                    new_map.insert(name, self.gc_dfs(var));
                }
                let new_outer = outer_clone.map(|val| self.gc_dfs(val));
                RuntimeNode::Environment(env_name_clone, new_map, new_outer)
            }
            RuntimeNode::Pair(car, cdr) => {
                let (car_val, cdr_val) = (*car, *cdr);
                let new_car = self.gc_dfs(car_val);
                let new_cdr = self.gc_dfs(cdr_val);
                RuntimeNode::Pair(new_car, new_cdr)
            }
            _ => self.get_node(false, dst_length).clone(),
        };
        let dst_area = self.get_area_mut(false);
        dst_area[dst_length] = content;
        dst_length
    }

    /// Insert a node into GC area.
    ///
    /// GC area must have enough space to insert the node. You should not use
    /// this unless you want to pin some variables to GC area.
    fn new_node(&mut self, node: RuntimeNode) -> usize {
        let result = self.get_free();
        assert!(result < self.size);
        self.get_area_mut(true).push(node);
        result
    }

    /// Perform GC and insert a node into GC area.
    pub fn new_node_with_gc(&mut self, node: RuntimeNode) -> usize {
        self.try_gc();
        self.new_node(node)
    }
}

// New and delete
impl Runtime {
    pub fn new(size: usize) -> Runtime {
        Runtime {
            stack: vec![],
            areas: (Vec::with_capacity(size), Vec::with_capacity(size)),
            size,
            roots: HashMap::new(),
            packages: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.roots.clear();
        self.stack.clear();
        self.packages.clear();
        self.areas.0.clear();
        self.areas.1.clear();
    }

    /// Called when there is an error.
    pub fn error(&mut self, msg: &str) {
        log_error(msg);
    }
}

// Manipulate root nodes
impl Runtime {
    pub fn add_root(&mut self, name: String, value: usize) {
        assert!(!self.roots.contains_key(&name));
        self.roots.insert(name, value);
    }
    pub fn set_root(&mut self, name: String, value: usize) {
        self.roots.insert(name, value);
    }
    pub fn remove_root(&mut self, name: &str) -> usize {
        self.roots.remove(name).unwrap()
    }

    pub fn get_root(&self, name: &str) -> usize {
        *self.roots.get(name).unwrap()
    }
}

// Getter
impl Runtime {
    fn get_area(&self, active: bool) -> &Vec<RuntimeNode> {
        if active {
            self.areas.0.as_ref()
        } else {
            self.areas.1.as_ref()
        }
    }
    fn get_area_mut(&mut self, active: bool) -> &mut Vec<RuntimeNode> {
        if active {
            self.areas.0.as_mut()
        } else {
            self.areas.1.as_mut()
        }
    }

    pub fn get_free(&self) -> usize {
        self.get_area(true).len()
    }
    pub fn get_size(&self) -> usize {
        self.size
    }
    pub fn get_node(&self, active: bool, index: usize) -> &RuntimeNode {
        self.get_area(active).get(index).unwrap()
    }

    pub fn get_node_mut(&mut self, active: bool, index: usize) -> &mut RuntimeNode {
        self.get_area_mut(active).get_mut(index).unwrap()
    }
    /// Get the underlying C function pointer of a closure.
    pub fn get_c_func(&self, index: usize) -> Result<Option<CVoidFunc>, RuntimeError> {
        if let RuntimeNode::Closure(c) = self.get_node(true, index) {
            Ok(Some(c.body))
        } else {
            Err(RuntimeError::new(format!(
                "{} is not a number",
                self.display_node_idx(index)
            )))
        }
    }
    pub fn get_number(&self, index: usize) -> Result<Number, RuntimeError> {
        if let RuntimeNode::Number(val) = self.get_node(true, index) {
            Ok(val.clone())
        } else {
            Err(RuntimeError::new(format!(
                "{} is not a number",
                self.display_node_idx(index)
            )))
        }
    }
    pub fn get_symbol(&self, index: usize) -> Result<Symbol, RuntimeError> {
        if let RuntimeNode::Symbol(val) = self.get_node(true, index) {
            Ok(val.clone())
        } else {
            Err(RuntimeError::new(format!(
                "{} is not a symbol",
                self.display_node_idx(index)
            )))
        }
    }
    pub fn get_pair(&self, index: usize) -> Result<(usize, usize), RuntimeError> {
        if let RuntimeNode::Pair(car, cdr) = self.get_node(true, index) {
            Ok((*car, *cdr))
        } else {
            Err(RuntimeError::new(format!(
                "{} is not a pair",
                self.display_node_idx(index)
            )))
        }
    }
}

// Environment manipulation
impl Runtime {
    pub fn new_env(&mut self, name: String, mut outer: usize) -> usize {
        self.push(outer);

        self.try_gc();

        outer = self.pop();
        self.new_node(RuntimeNode::Environment(name, HashMap::new(), Some(outer)))
    }

    pub fn current_env(&self) -> usize {
        let env_name = "__cur_env";
        *self.roots.get(env_name).unwrap()
    }

    pub fn top_env(&mut self) -> usize {
        let cur_name = "__cur_env";
        let top_name = "__top_env";
        assert!(!self.roots.contains_key(cur_name));
        assert!(!self.roots.contains_key(top_name));
        let node = self.new_node_with_gc(RuntimeNode::Environment(
            "top".to_string(),
            HashMap::new(),
            None,
        ));
        self.roots.insert(top_name.to_string(), node);
        self.roots.insert(cur_name.to_string(), node);
        node
    }

    pub fn get_cur_env(&self, idx: usize, key: &String) -> Option<usize> {
        if let RuntimeNode::Environment(_, map, _) = self.get_node(true, idx) {
            map.get(key).copied()
        } else {
            log_error(format!(
                "Expect an environment, found {}",
                self.display_node_idx(idx),
            ));
            None
        }
    }

    pub fn move_to_env(&mut self, env: usize) {
        if let RuntimeNode::Environment(_, _, _) = self.get_node(true, env) {
            self.set_root("__cur_env".to_string(), env);
        } else {
            panic!("Not an environment")
        }
    }

    pub fn get_outer_env(&self, idx: usize) -> Option<usize> {
        if let RuntimeNode::Environment(_, _, outer) = self.get_node(true, idx) {
            *outer
        } else {
            panic!("Not an environment")
        }
    }

    pub fn insert_cur_env(&mut self, idx: usize, key: &String, value: usize) {
        if let RuntimeNode::Environment(_, map, _) = self.get_node_mut(true, idx) {
            map.insert(key.to_string(), value);
        } else {
            panic!("Not an environment")
        }
    }
}

// Closures
impl Runtime {
    /// Get a closure from the stack, move to an environment whose outer is its `env`.
    fn move_to_closure_env(&mut self, cid: usize) -> Result<Closure, RuntimeError> {
        if let RuntimeNode::Closure(c) = self.get_node(true, cid) {
            let c = c.clone();
            // Construct and move to an environment.
            let env = self.new_env("closure".to_string(), c.env);
            self.move_to_env(env);
            Ok(c)
        } else {
            Err(RuntimeError::new(format!(
                "{} is not a closure",
                self.display_node_idx(cid)
            )))
        }
    }

    /// Push elements of a list to the stack. The list is at the top when the
    /// function is called. The stack after the function is called should be
    /// `(top) nargs elem1 elem2 ...`
    pub fn list_to_stack(&mut self) -> Result<(), RuntimeError> {
        let mut list = self.pop();
        if let Ok(Symbol::Nil) = self.get_symbol(list) {
            Number::Int(0).load_to(self).unwrap();
            return Ok(());
        }
        let mut elems = vec![];
        loop {
            let (car, cdr) = self.get_pair(list)?;
            list = cdr;
            elems.push(car);
            if let Ok(Symbol::Nil) = self.get_symbol(list) {
                for elem in elems.iter().rev() {
                    self.push(*elem);
                }
                Number::Int(elems.len() as i64).load_to(self).unwrap();
                return Ok(());
            }
        }
    }

    /// Pop the arguments from the stack and save them in a new environment.
    ///
    /// The stack before the function is called should be
    /// `(top) nargs operand0 operand1 ...`, where `operand0` has name
    /// `#0_func_{func_id}`, `operand1` has name `#1_func_{func_id}`, etc.
    pub fn prepare_args(&mut self, cid: usize) -> Result<(), RuntimeError> {
        let c = self.move_to_closure_env(cid)?;

        let idx = self.pop();
        let nparams = usize::try_from(self.get_number(idx)?)?;

        if (!c.variadic && c.nargs != nparams) || (c.variadic && c.nargs > nparams + 1) {
            return Err(RuntimeError::new(format!(
                "arity mismatch: expect {}, found {}",
                c.nargs, nparams
            )));
        }

        if c.nargs > 0 {
            // Add arguments to the environment.
            for i in 0..c.nargs - 1 {
                let value = self.pop();
                self.current_env()
                    .define(&format!("#{i}_func_{}", c.name), value, self);
            }

            if c.variadic {
                if c.nargs <= nparams {
                    // Zip the rest of the arguments (args[c.nargs-1..nparams])
                    // if the closure is variadic.
                    self.zip_stack_nodes(nparams - c.nargs + 1);
                } else {
                    // Load a '() as the last argument.
                    Symbol::Nil.load_to(self).unwrap();
                }
            }

            // Add the last argument.
            let last = self.pop();
            self.current_env()
                .define(&format!("#{}_func_{}", c.nargs - 1, c.name), last, self);
        }

        Ok(())
    }
}

// Misc
impl Runtime {
    pub fn node_vec_from_stack(&mut self, nargs: usize) -> Vec<RuntimeNode> {
        let mut operands = vec![];
        for _ in 0..nargs {
            let idx = self.pop();
            let node = self.get_node(true, idx).clone();
            operands.push(node);
        }
        operands
    }
    pub fn zip_stack_nodes(&mut self, nargs: usize) {
        // (top) a1 a2 ... an -> (top) an ... a2 a1
        let mut nodes = Vec::with_capacity(nargs);
        for _ in 0..nargs {
            nodes.push(self.pop());
        }
        for node in nodes.into_iter() {
            self.push(node);
        }

        Symbol::Nil.load_to(self).unwrap();
        for _ in 0..nargs {
            // (top) (a_{k+1} ... a_n) a_k a_{k-1} ... a_2 a_1
            self.swap();
            // (top) a_k (a_{k+1} ... a_n) a_{k-1} ... a_2 a_1
            self.new_pair();
            // (top) (a_k a_{k+1} ... a_n) a_{k-1} ... a_2 a_1
        }
    }
    fn pop_as_node(&mut self) -> RuntimeNode {
        let index = self.pop();
        self.get_node(true, index).clone()
    }

    pub fn copy_node(&mut self, active: bool, src: usize, dst: usize) {
        let area = self.get_area_mut(active);
        let src_val = area.get(src).unwrap();
        area[dst] = src_val.clone();
    }

    pub fn set_car(
        &mut self,
        active: bool,
        index: usize,
        target: usize,
    ) -> Result<(), RuntimeError> {
        let area = self.get_area_mut(active);
        let box_val = area.get_mut(index).unwrap();
        if let RuntimeNode::Pair(car, _) = box_val {
            *car = target;
            Ok(())
        } else {
            Err(RuntimeError::new(format!(
                "{} is not a pair",
                self.display_node_idx(index)
            )))
        }
    }

    pub fn set_cdr(
        &mut self,
        active: bool,
        index: usize,
        target: usize,
    ) -> Result<(), RuntimeError> {
        let area = self.get_area_mut(active);
        let box_val = area.get_mut(index).unwrap();
        if let RuntimeNode::Pair(_, cdr) = box_val {
            *cdr = target;
            Ok(())
        } else {
            Err(RuntimeError::new(format!(
                "{} is not a pair",
                self.display_node_idx(index)
            )))
        }
    }

    fn get_printable_node(
        &self,
        index: usize,
        list_index: usize,
        visited: &mut HashMap<usize, PrintableNode>,
    ) -> PrintableNode {
        if visited.contains_key(&index) {
            return visited.get(&index).unwrap().clone();
        }
        visited.insert(index, PrintableNode::String(format!("#{list_index}#")));
        let result = match self.get_node(true, index) {
            RuntimeNode::Symbol(Symbol::Nil) => PrintableNode::Nil,
            RuntimeNode::Symbol(sym) => sym.to_string().into(),
            RuntimeNode::Number(num) => num.to_string().into(),
            RuntimeNode::Pair(car, cdr) => {
                let car = self.get_printable_node(*car, list_index + 1, visited);
                let cdr = self.get_printable_node(*cdr, list_index + 1, visited);
                PrintableNode::Pair(car.into(), cdr.into())
            }
            RuntimeNode::BrokenHeart(dst) => format!("<BrokenHeart {dst}>").into(),
            RuntimeNode::Closure(Closure { env, nargs, .. }) => {
                format!("<Closure env: {env}, nargs: {nargs}>").into()
            }
            RuntimeNode::Environment(name, map, outer) => {
                let mut result = format!("<Env {name}: ");
                for (k, v) in map {
                    result += &format!("{k}={v}, ");
                }
                if let Some(env) = outer {
                    result += &format!("; outer = {env}");
                }
                format!("{result}>").into()
            }
        };
        visited.insert(index, result.clone());
        result
    }

    pub fn display_node_idx(&self, index: usize) -> String {
        format!("{}", self.get_printable_node(index, 0, &mut HashMap::new()))
    }

    /// Create a pair using the two elements from the stack. The first element
    /// popped is `car` and the second one is `cdr`.
    pub fn new_pair(&mut self) {
        self.try_gc();
        let car = self.pop();
        let cdr = self.pop();
        let pair = self.new_node(RuntimeNode::Pair(car, cdr));
        self.push(pair);
    }

    pub fn node_eq(&self, a: usize, b: usize) -> bool {
        match (self.get_node(true, a), self.get_node(true, b)) {
            (RuntimeNode::Symbol(a), RuntimeNode::Symbol(b)) => a == b,
            (RuntimeNode::Number(a), RuntimeNode::Number(b)) => a == b,
            (RuntimeNode::Pair(a, b), RuntimeNode::Pair(c, d)) => {
                self.node_eq(*a, *c) && self.node_eq(*b, *d)
            }
            (RuntimeNode::Environment(a, b, c), RuntimeNode::Environment(d, e, f)) => {
                a == d && b == e && c == f
            }
            (RuntimeNode::Closure(a), RuntimeNode::Closure(b)) => a == b,
            (RuntimeNode::BrokenHeart(a), RuntimeNode::BrokenHeart(b)) => a == b,
            (_, _) => false,
        }
    }
}
