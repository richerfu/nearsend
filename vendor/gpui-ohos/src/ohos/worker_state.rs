//! Kernel scheduling state for this process's worker threads.
use std::{fs::File, io, os::unix::fs::FileExt};

pub(crate) struct WorkerState(File);

impl WorkerState {
    /// Open on the worker itself; the fd stays bound to that task across reads.
    pub(crate) fn current() -> io::Result<Self> {
        File::open("/proc/thread-self/stat").map(Self)
    }

    pub(crate) fn is_blocked(&self) -> io::Result<bool> {
        let mut bytes = [0; 256];
        let len = self.0.read_at(&mut bytes, 0)?;
        let state = scheduler_state(&bytes[..len])
            .ok_or_else(|| io::Error::other("Missing kernel worker state"))?;
        // R includes both executing and runnable-but-preempted threads. Neither
        // needs a replacement. S/D are sleeping / uninterruptible waits.
        Ok(matches!(state, b'S' | b'D'))
    }
}

fn scheduler_state(stat: &[u8]) -> Option<u8> {
    // comm is parenthesized and may itself contain spaces or ')'.
    let end = stat.iter().rposition(|byte| *byte == b')')?;
    stat.get(end + 2).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_kernel_state_without_splitting_the_thread_name() {
        assert_eq!(scheduler_state(b"7 (worker ) name) R 1 2"), Some(b'R'));
        assert_eq!(scheduler_state(b"7 (worker) S 1 2"), Some(b'S'));
        assert_eq!(scheduler_state(b"7 (worker) D 1 2"), Some(b'D'));
        assert_eq!(scheduler_state(b""), None);
    }
}
