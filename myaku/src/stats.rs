//! The last minute of one measurement of one process.

/// How many samples a window keeps: one a second for a minute.
pub const WINDOW: usize = 60;

/// A ring of the last [`WINDOW`] values, `None` where there was nothing to
/// measure (the process had not started, had exited, or did not report it),
/// and their running sum, which ranks the processes.
#[derive(Clone, Debug)]
pub struct StatWindow {
    items: [Option<f64>; WINDOW],
    /// Where the newest value is.
    newest: usize,
    sum: f64,
}

impl Default for StatWindow {
    fn default() -> Self {
        Self {
            items: [None; WINDOW],
            newest: WINDOW - 1,
            sum: 0.0,
        }
    }
}

impl StatWindow {
    /// Adds the newest value, dropping the oldest.
    pub fn push(&mut self, value: Option<f64>) {
        self.newest = (self.newest + 1) % WINDOW;
        if let Some(old) = self.items[self.newest] {
            self.sum -= old;
        }
        if let Some(new) = value {
            self.sum += new;
        }
        self.items[self.newest] = value;
    }

    /// The values, oldest first.
    pub fn values(&self) -> impl Iterator<Item = Option<f64>> + '_ {
        let start = (self.newest + 1) % WINDOW;
        (0..WINDOW).map(move |i| self.items[(start + i) % WINDOW])
    }

    pub fn latest(&self) -> Option<f64> {
        self.items[self.newest]
    }

    /// The sum of the values in the window. Floating-point drift from adding
    /// and removing is kept from going below zero.
    pub fn sum(&self) -> f64 {
        self.sum.max(0.0)
    }

    /// Whether every value in the window is missing.
    pub fn is_empty(&self) -> bool {
        self.items.iter().all(Option::is_none)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_empty() {
        let w = StatWindow::default();
        assert!(w.is_empty());
        assert_eq!(w.latest(), None);
        assert_eq!(w.values().count(), WINDOW);
        assert_eq!(w.sum(), 0.0);
    }

    #[test]
    fn keeps_the_values_oldest_first() {
        let mut w = StatWindow::default();
        w.push(Some(1.0));
        w.push(None);
        w.push(Some(3.0));
        let v: Vec<_> = w.values().collect();
        assert_eq!(&v[WINDOW - 3..], &[Some(1.0), None, Some(3.0)]);
        assert!(v[..WINDOW - 3].iter().all(Option::is_none));
        assert_eq!(w.latest(), Some(3.0));
        assert_eq!(w.sum(), 4.0);
    }

    #[test]
    fn drops_the_oldest_once_full() {
        let mut w = StatWindow::default();
        for i in 0..WINDOW + 2 {
            w.push(Some(i as f64));
        }
        let v: Vec<_> = w.values().map(Option::unwrap).collect();
        assert_eq!(v[0], 2.0);
        assert_eq!(v[WINDOW - 1], (WINDOW + 1) as f64);
        assert_eq!(w.sum(), (2..WINDOW + 2).sum::<usize>() as f64);
    }

    #[test]
    fn becomes_empty_after_a_minute_of_nothing() {
        let mut w = StatWindow::default();
        w.push(Some(1.0));
        for _ in 0..WINDOW - 1 {
            w.push(None);
            assert!(!w.is_empty());
        }
        w.push(None);
        assert!(w.is_empty());
        assert_eq!(w.sum(), 0.0);
    }
}
