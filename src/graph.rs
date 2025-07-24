//! Functionality to print the graph representation of the environment and nodes.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    fmt::{self, Display},
    hash::{DefaultHasher, Hash, Hasher},
    rc::Rc,
};

use crate::node::{Node, NodeEnv};

struct PrintEnv {
    id: *const NodeEnv,
    vars: HashMap<String, *const Node>,
    outer: Option<*const NodeEnv>,
    name: String,
}

pub struct PrintState {
    nodes: HashMap<*const Node, Rc<RefCell<Node>>>,
    envs: HashMap<*const NodeEnv, PrintEnv>,
    name: String,
}

impl PrintEnv {
    fn new(
        env: Rc<RefCell<NodeEnv>>,
        nodes: &mut HashMap<*const Node, Rc<RefCell<Node>>>,
        frontier: &mut VecDeque<Rc<RefCell<NodeEnv>>>,
        visited_env: &mut HashSet<*const NodeEnv>,
    ) -> PrintEnv {
        fn add_env(
            env: &Rc<RefCell<NodeEnv>>,
            frontier: &mut VecDeque<Rc<RefCell<NodeEnv>>>,
            visited: &mut HashSet<*const NodeEnv>,
        ) {
            if !visited.contains(&(env.as_ptr() as *const NodeEnv)) {
                frontier.push_back(env.clone());
            }
        }
        fn add_node(
            node: &Rc<RefCell<Node>>,
            ptr: *const Node,
            nodes: &mut HashMap<*const Node, Rc<RefCell<Node>>>,
            frontier: &mut VecDeque<Rc<RefCell<NodeEnv>>>,
            visited: &mut HashSet<*const NodeEnv>,
        ) {
            if nodes.contains_key(&ptr) {
                return;
            }
            nodes.insert(ptr, node.clone());
            match &*node.borrow() {
                Node::Pair(car, cdr) => {
                    add_node(car, car.as_ptr() as *const Node, nodes, frontier, visited);
                    add_node(cdr, cdr.as_ptr() as *const Node, nodes, frontier, visited)
                }
                Node::Procedure(_, _, env) => add_env(env, frontier, visited),
                _ => (),
            }
        }
        let id: *const NodeEnv = env.as_ptr();
        let outer = env.borrow().outer.as_ref().map(|x| {
            add_env(x, frontier, visited_env);
            x.as_ptr() as *const NodeEnv
        });
        let vars: HashMap<String, *const Node> = env
            .borrow()
            .map
            .iter()
            .map(|(k, v)| {
                let vptr = v.as_ptr() as *const Node;
                add_node(v, vptr, nodes, frontier, visited_env);
                (k.clone(), vptr)
            })
            .collect();
        PrintEnv {
            id,
            vars,
            outer,
            name: env.borrow().name.clone(),
        }
    }
}

impl PrintState {
    pub fn new(cur_env: Rc<RefCell<NodeEnv>>, name: String) -> PrintState {
        let mut nodes = HashMap::new();
        let mut envs = HashMap::new();
        let mut visited_env = HashSet::new();
        let mut frontier = VecDeque::new();
        frontier.push_back(cur_env);
        while let Some(front) = frontier.pop_front() {
            envs.insert(
                front.as_ptr() as *const NodeEnv,
                PrintEnv::new(front, &mut nodes, &mut frontier, &mut visited_env),
            );
        }
        PrintState { nodes, envs, name }
    }
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

impl Display for PrintEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Create subgraph for this environment
        writeln!(f, "\tsubgraph cluster_{:?} {{", self.id)?;
        writeln!(f, "\t\tlabel=\"Env {}\"", self.name)?;
        writeln!(f, "\t\tstyle=filled;")?;
        writeln!(f, "\t\tcolor=lightgrey;")?;
        // Create an invisible node to represent the environment itself
        writeln!(
            f,
            "\t\tenv_node_{:?} [label=\"\", shape=point, style=invis];\n",
            self.id
        )?;
        // Add key-value pairs
        for (key, value) in &self.vars {
            let key_hash = calculate_hash(key);
            writeln!(
                f,
                "\t\tkey_{key_hash}_{value:?} [label=\"{key}\", shape=box];"
            )?;
            writeln!(f, "\t\tkey_{key_hash}_{value:?} -> node_{value:?};")?;
        }
        writeln!(f, "\t}}")?;
        // Add pointer to outer env
        if let Some(outer_ptr) = self.outer {
            writeln!(
                f,
                "\t\tenv_node_{:?} -> env_node_{:?} [label=\"outer\", style=dashed];",
                self.id, outer_ptr
            )?;
        }
        Ok(())
    }
}

impl Display for PrintState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        macro_rules! as_ptr {
            ($ty:tt, $container:expr, $node:expr) => {
                $container.get(&($node.as_ptr() as *const $ty)).unwrap()
            };
        }
        writeln!(f, "digraph {} {{\n", self.name)?;
        // Write nodes
        for (ptr, node) in &self.nodes {
            // Write the node itself
            writeln!(
                f,
                "\tnode_{:?} [label=\"{}\", shape=box]",
                ptr,
                node.borrow()
            )?;
            // Write arrows if there are any
            match &*node.borrow() {
                Node::Number(_) | Node::Symbol(_) | Node::SpecialForm(_) => (),
                Node::Pair(car, cdr) => {
                    writeln!(
                        f,
                        "\tnode_{:?} -> node_{:?}",
                        ptr,
                        as_ptr!(Node, self.nodes, car).as_ptr()
                    )?;
                    writeln!(
                        f,
                        "\tnode_{:?} -> node_{:?}",
                        ptr,
                        as_ptr!(Node, self.nodes, cdr).as_ptr()
                    )?;
                }
                Node::Procedure(_, _, env) => {
                    let eptr = as_ptr!(NodeEnv, self.envs, env).id;
                    writeln!(
                        f,
                        "\tnode_{ptr:?} -> env_node_{eptr:?}  [label=\"env\", style=dashed]"
                    )?;
                }
            };
        }
        // Write environments
        for env in self.envs.values() {
            writeln!(f, "{env}")?
        }
        writeln!(f, "}}")
    }
}
