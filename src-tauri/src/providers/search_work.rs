//! Test-only work counters. Release builds contain no counters or branches.
use std::cell::Cell;

#[derive(Clone, Copy)]
pub enum Provider {
    Apps,
    Files,
    Clipboard,
    System,
    Emoji,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Work {
    pub calls: [usize; 5],
    pub candidates: [usize; 5],
}

thread_local! {
    static WORK: Cell<Work> = Cell::new(Work::default());
}

pub fn record(provider: Provider, candidates: usize) {
    WORK.with(|work| {
        let mut value = work.get();
        value.calls[provider as usize] += 1;
        value.candidates[provider as usize] += candidates;
        work.set(value);
    });
}

pub fn take() -> Work {
    WORK.with(|work| work.replace(Work::default()))
}
