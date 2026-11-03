//! The last few seconds of a recording.
//!
//! While recording, every captured piece (an encoded frame or packet) is pushed here with its
//! time. Pieces older than the window are dropped as new ones arrive, so a recording can run for
//! as long as the person likes and still only holds the last 10 s (30 s at most). A byte budget
//! guards the memory too: a very large display cannot make the buffer grow past it.

use std::collections::VecDeque;

/// Shortest and longest window, in seconds.
pub const MIN_SECONDS: u32 = 1;
pub const MAX_SECONDS: u32 = 30;
pub const DEFAULT_SECONDS: u32 = 10;

/// How many seconds the buffer keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window(u32);

impl Window {
    /// Clamped to 1..=30, so a typo in a setting cannot ask for an hour of memory.
    pub fn seconds(n: u32) -> Self {
        Self(n.clamp(MIN_SECONDS, MAX_SECONDS))
    }

    pub fn as_secs(self) -> u32 {
        self.0
    }

    pub fn as_millis(self) -> u64 {
        u64::from(self.0) * 1000
    }
}

impl Default for Window {
    fn default() -> Self {
        Self(DEFAULT_SECONDS)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PushError {
    #[error("a piece arrived from the past ({got} ms after {last} ms)")]
    OutOfOrder { last: u64, got: u64 },
}

struct Entry<T> {
    at_ms: u64,
    weight: usize,
    item: T,
}

pub struct RollingBuffer<T> {
    window: Window,
    max_weight: usize,
    items: VecDeque<Entry<T>>,
    weight: usize,
    /// Pieces dropped because of the byte budget rather than age: the clip is shorter than the
    /// window, and the person can be told why.
    squeezed: u64,
}

impl<T> RollingBuffer<T> {
    /// `max_weight` is the byte budget (or any other unit the pieces are weighed in).
    pub fn new(window: Window, max_weight: usize) -> Self {
        Self {
            window,
            max_weight,
            items: VecDeque::new(),
            weight: 0,
            squeezed: 0,
        }
    }

    /// Add a piece taken at `at_ms` (milliseconds on any steady clock, never going backwards).
    /// Returns how many older pieces were dropped to make room.
    pub fn push(&mut self, at_ms: u64, weight: usize, item: T) -> Result<usize, PushError> {
        if let Some(last) = self.items.back() {
            if at_ms < last.at_ms {
                return Err(PushError::OutOfOrder {
                    last: last.at_ms,
                    got: at_ms,
                });
            }
        }
        self.items.push_back(Entry {
            at_ms,
            weight,
            item,
        });
        self.weight += weight;
        let mut dropped = 0;
        // Too old: more than the window before the newest piece.
        while let Some(first) = self.items.front() {
            if at_ms - first.at_ms > self.window.as_millis() {
                self.pop_front();
                dropped += 1;
            } else {
                break;
            }
        }
        // Too heavy: keep at least the newest piece, even if it alone is over the budget.
        while self.weight > self.max_weight && self.items.len() > 1 {
            self.pop_front();
            dropped += 1;
            self.squeezed += 1;
        }
        Ok(dropped)
    }

    fn pop_front(&mut self) {
        if let Some(e) = self.items.pop_front() {
            self.weight -= e.weight;
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn weight(&self) -> usize {
        self.weight
    }

    /// How much time the buffer holds right now.
    pub fn span_ms(&self) -> u64 {
        match (self.items.front(), self.items.back()) {
            (Some(a), Some(b)) => b.at_ms - a.at_ms,
            _ => 0,
        }
    }

    /// Did the byte budget, not the window, cut the start of the clip?
    pub fn was_squeezed(&self) -> bool {
        self.squeezed > 0
    }

    pub fn window(&self) -> Window {
        self.window
    }

    /// The pieces in order, with their times relative to the first one (so the clip starts at 0).
    pub fn take(&mut self) -> Vec<(u64, T)> {
        let start = self.items.front().map(|e| e.at_ms).unwrap_or(0);
        self.weight = 0;
        self.squeezed = 0;
        self.items
            .drain(..)
            .map(|e| (e.at_ms - start, e.item))
            .collect()
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.weight = 0;
        self.squeezed = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill(b: &mut RollingBuffer<u32>, from_ms: u64, to_ms: u64, step: u64) {
        let mut t = from_ms;
        while t <= to_ms {
            b.push(t, 10, t as u32).unwrap();
            t += step;
        }
    }

    #[test]
    fn the_window_is_clamped_and_defaults_to_ten_seconds() {
        assert_eq!(Window::default().as_secs(), 10);
        assert_eq!(Window::seconds(0).as_secs(), 1);
        assert_eq!(Window::seconds(7).as_secs(), 7);
        assert_eq!(Window::seconds(3600).as_secs(), 30);
        assert_eq!(Window::seconds(30).as_millis(), 30_000);
    }

    #[test]
    fn a_long_recording_keeps_only_the_last_window() {
        let mut b = RollingBuffer::new(Window::seconds(10), usize::MAX);
        // two minutes at 10 pieces a second
        fill(&mut b, 0, 120_000, 100);
        assert_eq!(b.span_ms(), 10_000);
        // 10 s inclusive of both ends at 100 ms steps
        assert_eq!(b.len(), 101);
        let clip = b.take();
        assert_eq!(clip.first().unwrap().0, 0, "the clip starts at zero");
        assert_eq!(clip.last().unwrap().0, 10_000);
        assert_eq!(clip.first().unwrap().1, 110_000);
        assert!(b.is_empty());
        assert_eq!(b.weight(), 0);
    }

    #[test]
    fn a_short_recording_is_kept_whole() {
        let mut b = RollingBuffer::new(Window::seconds(10), usize::MAX);
        fill(&mut b, 5_000, 7_000, 500);
        assert_eq!(b.len(), 5);
        assert_eq!(b.span_ms(), 2_000);
        assert!(!b.was_squeezed());
    }

    #[test]
    fn the_byte_budget_drops_the_oldest_and_says_so() {
        let mut b = RollingBuffer::new(Window::seconds(30), 100);
        // each piece weighs 10, so ten fit
        fill(&mut b, 0, 1_900, 100);
        assert_eq!(b.len(), 10);
        assert_eq!(b.weight(), 100);
        assert!(b.was_squeezed());
        assert_eq!(b.take().first().unwrap().0, 0);
        assert!(!b.was_squeezed(), "taking the clip starts a new count");
    }

    #[test]
    fn one_huge_piece_is_still_kept() {
        let mut b = RollingBuffer::new(Window::seconds(10), 5);
        b.push(0, 1000, "keyframe").unwrap();
        assert_eq!(b.len(), 1);
        b.push(100, 1000, "next").unwrap();
        assert_eq!(b.len(), 1, "only the newest fits");
        assert_eq!(b.take()[0].1, "next");
    }

    #[test]
    fn time_never_goes_backwards() {
        let mut b = RollingBuffer::new(Window::default(), usize::MAX);
        b.push(1_000, 1, 1).unwrap();
        b.push(1_000, 1, 2).unwrap();
        let err = b.push(999, 1, 3).unwrap_err();
        assert_eq!(
            err,
            PushError::OutOfOrder {
                last: 1_000,
                got: 999
            }
        );
        assert_eq!(b.len(), 2);
    }

    #[test]
    fn clearing_starts_over() {
        let mut b = RollingBuffer::new(Window::default(), usize::MAX);
        fill(&mut b, 0, 500, 100);
        b.clear();
        assert!(b.is_empty());
        assert_eq!(b.span_ms(), 0);
        assert_eq!(b.weight(), 0);
        // a fresh recording may use a clock that restarted
        b.push(0, 1, 1).unwrap();
    }
}
