// SPDX-License-Identifier: GPL-3.0-or-later
use std::collections::VecDeque;

/// Fixed-length rolling series for charts. `None` samples stay as gaps.
#[derive(Debug, Clone)]
pub struct Series {
    buf: VecDeque<Option<f32>>,
    cap: usize,
}

impl Series {
    pub fn new(cap: usize) -> Self {
        Self { buf: VecDeque::with_capacity(cap), cap: cap.max(1) }
    }
    pub fn push(&mut self, v: Option<f32>) {
        if self.buf.len() == self.cap {
            self.buf.pop_front();
        }
        self.buf.push_back(v);
    }
    pub fn iter(&self) -> impl Iterator<Item = Option<f32>> + '_ {
        self.buf.iter().copied()
    }
    pub fn len(&self) -> usize {
        self.buf.len()
    }
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
    pub fn cap(&self) -> usize {
        self.cap
    }
    pub fn last(&self) -> Option<f32> {
        self.buf.back().copied().flatten()
    }
    pub fn max(&self) -> Option<f32> {
        self.iter().flatten().reduce(f32::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolls_and_keeps_gaps() {
        let mut s = Series::new(3);
        for v in [Some(1.0), None, Some(3.0), Some(4.0)] {
            s.push(v);
        }
        assert_eq!(s.iter().collect::<Vec<_>>(), vec![None, Some(3.0), Some(4.0)]);
        assert_eq!((s.last(), s.max(), s.len()), (Some(4.0), Some(4.0), 3));
    }
}
