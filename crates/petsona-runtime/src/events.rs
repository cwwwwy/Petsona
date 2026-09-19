//! Runtime event primitives shared by the worker and native hosts.

use std::sync::mpsc::Sender;

use crate::commands::RuntimeCommand;

/// A cheap, cloneable wake handle for platform event loops.
#[derive(Clone)]
pub struct RuntimeWaker {
    command_tx: Sender<RuntimeCommand>,
}

impl RuntimeWaker {
    pub(crate) fn new(command_tx: Sender<RuntimeCommand>) -> Self {
        Self { command_tx }
    }

    pub fn wake(&self) {
        let _ = self.command_tx.send(RuntimeCommand::Wake);
    }
}
