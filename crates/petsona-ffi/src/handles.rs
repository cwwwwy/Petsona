use petsona_runtime::engine::RuntimeEngine;

pub struct Engine {
    pub(crate) runtime: RuntimeEngine,
    pub(crate) terminal_error: Option<String>,
}

impl Engine {
    pub fn is_faulted(&self) -> bool {
        self.terminal_error.is_some() || self.runtime.snapshot().faulted
    }

    pub fn mark_faulted(&mut self, message: impl Into<String>) {
        self.terminal_error = Some(message.into());
    }
}
