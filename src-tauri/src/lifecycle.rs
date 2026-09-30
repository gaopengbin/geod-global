//! Quiesce IPC before explicit exit, keeping the event loop free to render.
use std::sync::{Arc, Condvar, Mutex};

#[derive(Default)]
struct State {
    exiting: bool,
    finished: bool,
    commands: usize,
}

#[derive(Clone, Default)]
pub struct DesktopLifecycle(Arc<Inner>);

#[derive(Default)]
struct Inner {
    state: Mutex<State>,
    settled: Condvar,
}

pub struct CommandGuard {
    lifecycle: DesktopLifecycle,
}

impl DesktopLifecycle {
    pub fn enter(&self) -> Result<CommandGuard, String> {
        let mut state = self
            .0
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if state.exiting {
            return Err(
                "The application is exiting; wait for it to finish before reopening".into(),
            );
        }
        state.commands += 1;
        Ok(CommandGuard {
            lifecycle: self.clone(),
        })
    }

    pub fn begin_exit(&self) -> bool {
        let mut state = self
            .0
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if state.exiting {
            return false;
        }
        state.exiting = true;
        true
    }

    pub fn exiting(&self) -> bool {
        self.0
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .exiting
    }

    pub fn finished(&self) -> bool {
        self.0
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .finished
    }

    pub fn finish(&self) {
        self.0
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .finished = true;
    }

    /// Run on a blocking pool, never on Tauri's window thread.
    pub fn drain_commands(&self) {
        let mut state = self
            .0
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        while state.commands != 0 {
            state = self
                .0
                .settled
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
    }
}

impl Drop for CommandGuard {
    fn drop(&mut self) {
        let mut state = self
            .lifecycle
            .0
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.commands -= 1;
        if state.commands == 0 {
            self.lifecycle.0.settled.notify_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn exit_rejects_new_commands_and_drains_in_flight_commands_once() {
        let lifecycle = DesktopLifecycle::default();
        let command = lifecycle.enter().unwrap();
        assert!(lifecycle.begin_exit());
        assert!(!lifecycle.begin_exit());
        assert!(lifecycle.enter().is_err());
        let (send, receive) = mpsc::channel();
        let worker_lifecycle = lifecycle.clone();
        let worker = std::thread::spawn(move || {
            worker_lifecycle.drain_commands();
            worker_lifecycle.finish();
            send.send(()).unwrap();
        });
        assert!(receive.recv_timeout(Duration::from_millis(30)).is_err());
        assert!(!lifecycle.finished());
        drop(command);
        receive.recv_timeout(Duration::from_secs(2)).unwrap();
        worker.join().unwrap();
        assert!(lifecycle.finished());
        assert!(lifecycle.enter().is_err());
    }
}
