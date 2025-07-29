//! The environment module.

/// Environment.
///
/// An environment consists of a variable mapping and a pointer to an outer
/// environment. `define`, `get` and `set!` operators can be derived once you
/// provide the correct components.
pub trait Env<K, V, R>
where
    V: Clone,
{
    /// Insert key-value pair into current environment.
    fn insert_cur(&mut self, key: &K, value: V, runtime: &mut R);
    /// Query the current environment.
    fn get_cur(&self, key: &K, runtime: &R) -> Option<V>;
    /// Whether the current environmet has outer environment.
    fn has_outer(&self, runtime: &R) -> bool;
    /// Call `func` in outer environment. This function won't be called when
    /// `has_outer` returns `false`.
    fn do_in_outer<Out, F>(&self, func: F, runtime: &R) -> Out
    where
        F: Fn(&Self) -> Out,
        Self: Sized;
    /// Call mutable `func` in outer environment. This function won't be called
    /// when `has_outer` returns `false`.
    fn do_in_outer_mut<Out, F>(&mut self, func: F, runtime: &mut R) -> Out
    where
        F: Fn(&mut Self, &mut R) -> Out,
        Self: Sized;

    /// Whether current environmet contains `key`.
    fn contains(&self, key: &K, runtime: &R) -> bool {
        self.get_cur(key, runtime).is_some()
    }
    /// `define` operator.
    fn define(&mut self, key: &K, value: V, runtime: &mut R) {
        self.insert_cur(key, value, runtime);
    }
    /// `get` operator.
    fn get(&self, key: &K, runtime: &R) -> Option<V>
    where
        Self: Sized,
    {
        match (self.get_cur(key, runtime), self.has_outer(runtime)) {
            (Some(value), _) => Some(value),
            (None, true) => self.do_in_outer(|env| env.get(key, runtime), runtime),
            _ => None,
        }
    }
    /// `set!` operator.
    fn set(&mut self, key: &K, value: V, runtime: &mut R) -> Option<V>
    where
        Self: Sized,
    {
        if self.contains(key, runtime) {
            self.insert_cur(key, value.clone(), runtime);
            Some(value)
        } else if self.has_outer(runtime) {
            self.do_in_outer_mut(|env, r| env.set(key, value.clone(), r), runtime)
        } else {
            None
        }
    }
}
