//! The data structure of node and other compiled-time objects.

use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::{self, Display},
    rc::Rc,
    str::FromStr,
};

use crate::{
    env::Env,
    lexer::{Lexer, Number},
    nil,
    parser::Parse,
    symbol::{SpecialForm, Symbol},
};

/// Compile time environment.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct NodeEnv {
    /// Variable mapping.
    pub map: HashMap<String, Rc<RefCell<Node>>>,
    /// Outer environment. The value is `None` if the environment is global.
    pub outer: Option<Rc<RefCell<NodeEnv>>>,
    /// The name of the environment that will be printed on DOT graphs.
    pub name: String,
}

impl Display for NodeEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Env {} {{", self.name)?;
        for (key, value) in &self.map {
            writeln!(f, "  {}: {}", key, value.borrow())?;
        }
        if let Some(outer) = &self.outer {
            writeln!(f, "  Outer: {}", outer.borrow())?;
        }
        writeln!(f, "}}")
    }
}

impl Env<String, Rc<RefCell<Node>>> for NodeEnv {
    fn top(_: &mut ()) -> Self {
        Self {
            map: HashMap::new(),
            outer: None,
            name: "Global".to_string(),
        }
    }
    fn contains(&self, key: &String, _: &()) -> bool {
        self.map.contains_key(key)
    }
    fn insert_cur(&mut self, key: &String, value: Rc<RefCell<Node>>, _: &mut ()) {
        self.map.insert(key.to_string(), value);
    }
    fn get_cur(&self, key: &String, _: &()) -> Option<Rc<RefCell<Node>>> {
        self.map.get(key).cloned()
    }
    fn has_outer(&self, _: &()) -> bool {
        self.outer.is_some()
    }
    fn do_in_outer<Out, F>(&self, func: F, _: &()) -> Out
    where
        F: Fn(&Self) -> Out,
        Self: Sized,
    {
        let outer = self.outer.clone().unwrap();
        func(&outer.borrow())
    }
    fn do_in_outer_mut<Out, F>(&mut self, func: F, _: &mut ()) -> Out
    where
        F: Fn(&mut Self, &mut ()) -> Out,
        Self: Sized,
    {
        let outer = self.outer.clone().unwrap();
        func(&mut outer.borrow_mut(), &mut ())
    }
}

impl NodeEnv {
    pub fn new(
        outer: Option<Rc<RefCell<NodeEnv>>>,
        map: HashMap<String, Rc<RefCell<Node>>>,
        name: &str,
    ) -> Self {
        Self {
            map,
            outer,
            name: name.to_string(),
        }
    }
}

/// The data structure of the node in reference counting graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// Symbols.
    Symbol(Symbol),
    /// Numbers.
    Number(Number),
    /// Pair of nodes.
    Pair(Rc<RefCell<Node>>, Rc<RefCell<Node>>),
    /// An item of special form.
    SpecialForm(SpecialForm),
    /// Procedure objects. This is what you get when you evaluate a `lambda`.
    /// The fields are:
    /// - Pattern
    /// - Function body (represented by a node)
    /// - Environment
    Procedure(Pattern, Rc<RefCell<Node>>, Rc<RefCell<NodeEnv>>),
}

pub type NodePair = (Rc<RefCell<Node>>, Rc<RefCell<Node>>);

impl Node {
    pub fn as_int(&self) -> Result<i64, String> {
        match self {
            Node::Number(Number::Int(num)) => Ok(*num),
            _ => Err(format!("{self} is not an integer")),
        }
    }

    pub fn as_num(&self) -> Result<Number, String> {
        match self {
            Node::Number(num) => Ok(num.clone()),
            _ => Err(format!("{self} is not a number")),
        }
    }

    pub fn as_user_symbol(&self) -> Result<String, String> {
        match self {
            Node::Symbol(Symbol::User(name)) => Ok(name.clone()),
            _ => Err(format!("{self} is not a user defined symbol")),
        }
    }

    pub fn as_pair(&self) -> Result<NodePair, String> {
        match self {
            Node::Pair(car, cdr) => Ok((car.clone(), cdr.clone())),
            _ => Err(format!("{self} is not a pair")),
        }
    }

    pub fn set_car(&mut self, value: Rc<RefCell<Node>>) -> Result<(), String> {
        match self {
            Node::Pair(car, _) => {
                *car = value;
                Ok(())
            }
            _ => Err(format!("{self} is not a pair")),
        }
    }

    pub fn set_cdr(&mut self, value: Rc<RefCell<Node>>) -> Result<(), String> {
        match self {
            Node::Pair(_, cdr) => {
                *cdr = value;
                Ok(())
            }
            _ => Err(format!("{self} is not a pair")),
        }
    }
}

/// The data structure of a pattern. A pattern is a improper list consists of
/// symbols.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    /// Symbols.
    Symbol(String),
    /// Pair of patterns.
    Pair(String, Box<Pattern>),
    /// Nil.
    Nil,
}

impl Pattern {
    pub fn is_proper_list(&self) -> bool {
        match self {
            Pattern::Nil => true,
            Pattern::Symbol(_) => false,
            Pattern::Pair(_, cdr) => cdr.is_proper_list(),
        }
    }
    pub fn vectorize(&self, lst: &mut Vec<String>) {
        match self {
            Pattern::Nil => {}
            Pattern::Symbol(sym) => lst.push(sym.clone()),
            Pattern::Pair(car, cdr) => {
                lst.push(car.clone());
                cdr.vectorize(lst);
            }
        }
    }
}

pub fn pattern_matching(
    pattern: &Pattern,
    actual: &[Rc<RefCell<Node>>],
    bindings: &mut HashMap<String, Rc<RefCell<Node>>>,
) -> Result<(), String> {
    match (pattern, actual) {
        (Pattern::Symbol(sym), x) => {
            let node = Node::from_iter(x.iter().cloned());
            bindings.insert(sym.to_string(), node.into());
            Ok(())
        }
        (Pattern::Pair(car, cdr), &[ref head, ref tail @ ..]) => {
            bindings.insert(car.clone(), head.clone());
            pattern_matching(cdr, tail, bindings)
        }
        (Pattern::Nil, []) => Ok(()),
        _ => Err(format!(
            "Parameter mismatch: expect {pattern}, got {actual:?}"
        )),
    }
}

impl From<Pattern> for Node {
    fn from(value: Pattern) -> Self {
        match value {
            Pattern::Symbol(str) => Node::Symbol(Symbol::User(str)),
            Pattern::Pair(car, cdr) => Node::Pair(
                Rc::new(RefCell::new(Node::Symbol(Symbol::User(car)))),
                Rc::new(RefCell::new(Self::from(*cdr))),
            ),
            Pattern::Nil => nil!(),
        }
    }
}

impl TryFrom<Rc<RefCell<Node>>> for Pattern {
    type Error = String;
    fn try_from(value: Rc<RefCell<Node>>) -> Result<Self, Self::Error> {
        match &*value.borrow() {
            Node::Symbol(Symbol::User(str)) => Ok(Self::Symbol(str.clone())),
            Node::Pair(car, cdr) => {
                if let Node::Symbol(Symbol::User(sym)) = &*car.borrow() {
                    Ok(Self::Pair(sym.clone(), Box::new(cdr.clone().try_into()?)))
                } else {
                    Err(format!("{} is not a list", value.borrow()))
                }
            }
            nil!() => Ok(Pattern::Nil),
            _ => Err(format!(
                "Can't transform node {} to pattern",
                value.borrow()
            )),
        }
    }
}

impl Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Node::from(self.clone()))
    }
}

impl Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut visited = HashMap::new();
        self.fmt_with_visited(f, &mut visited, 0)
    }
}

impl Node {
    fn fmt_with_visited(
        &self,
        f: &mut fmt::Formatter<'_>,
        visited: &mut HashMap<*const Node, usize>,
        id: usize,
    ) -> fmt::Result {
        match self {
            Node::Number(num) => write!(f, "{num}"),
            Node::SpecialForm(sym) => write!(f, "{sym}"),
            Node::Symbol(sym) => write!(f, "{sym}"),
            Node::Pair(car, cdr) => {
                let cdr_ptr = cdr.as_ptr() as *const Node;
                if let Some(prev_id) = visited.get(&cdr_ptr) {
                    return write!(f, "#{prev_id}#");
                }
                visited.insert(cdr_ptr, id);

                let car_ptr = car.as_ptr() as *const Node;
                if let Some(prev_id) = visited.get(&car_ptr) {
                    write!(f, "(#{prev_id}#")?;
                } else {
                    write!(f, "(")?;
                    car.borrow().fmt_with_visited(f, visited, id)?;
                    visited.insert(car_ptr, id);
                }

                let mut current = cdr.clone();
                let mut current_id = id;
                loop {
                    let next = {
                        let node = current.borrow();
                        match &*node {
                            Node::Pair(next_car, next_cdr) => {
                                let cdr_ptr = next_cdr.as_ptr() as *const Node;

                                if let Some(prev_id) = visited.get(&cdr_ptr) {
                                    write!(f, " . #{prev_id}#",)?;
                                    break;
                                }

                                let next_id = current_id + 1;
                                visited.insert(cdr_ptr, next_id);

                                let car_ptr = next_car.as_ptr() as *const Node;
                                if let Some(prev_id) = visited.get(&car_ptr) {
                                    write!(f, " #{prev_id}#",)?;
                                } else {
                                    write!(f, " ")?;
                                    next_car.borrow().fmt_with_visited(f, visited, next_id)?;
                                    visited.insert(car_ptr, next_id);
                                }
                                Some((next_cdr.clone(), next_id))
                            }
                            Node::Symbol(Symbol::Nil) => None,
                            Node::Number(_)
                            | Node::Symbol(_)
                            | Node::Procedure(_, _, _)
                            | Node::SpecialForm(_) => {
                                write!(f, " . {node}")?;
                                None
                            }
                        }
                    };

                    match next {
                        Some((next_cdr, next_id)) => {
                            current = next_cdr;
                            current_id = next_id;
                        }
                        None => break,
                    }
                }
                write!(f, ")")
            }
            Node::Procedure(pattern, body, _) => write!(f, "(λ {} {})", pattern, body.borrow()),
        }
    }
}

impl<T> FromIterator<T> for Node
where
    T: Into<Rc<RefCell<Node>>>,
{
    fn from_iter<It: IntoIterator<Item = T>>(iter: It) -> Self {
        let items: Vec<_> = iter.into_iter().collect();
        let mut cur = nil!();
        for value in items.into_iter().rev() {
            cur = Node::Pair(value.into(), cur.into())
        }
        cur
    }
}

impl From<Node> for Rc<RefCell<Node>> {
    fn from(value: Node) -> Self {
        RefCell::new(value).into()
    }
}

impl FromStr for Node {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut tokens = Lexer::new(s);
        let mut nodes = vec![];
        loop {
            if tokens.peek_next_token().1.is_none() {
                // Create a `(begin ...)` node
                return Ok(Node::Pair(
                    Node::SpecialForm(SpecialForm::Begin).into(),
                    Node::from_iter(nodes).into(),
                ));
            }
            nodes.push(Node::parse(&mut tokens).map_err(|e| e.to_string())?);
        }
    }
}
