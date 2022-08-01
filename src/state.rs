use std::sync::Arc;

pub(crate) enum State<T> {
    UnSet,
    Value(Arc<T>),
    Killed,
}

impl<T> Default for State<T> {
    fn default() -> Self {
        State::UnSet
    }
}
