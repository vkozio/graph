use std::time::Duration;

/// Wasm-safe timing probe for log-only measurements.
///
/// `std::time::Instant::now()` panics at runtime on `wasm32-unknown-unknown`,
/// but every use in this crate only feeds `log::info!` timings (a no-op
/// without an initialized logger). On wasm32 this therefore measures nothing
/// and reports zero, keeping one code path for all targets with no extra
/// dependency (no `web-time`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Timer {
    #[cfg(not(target_arch = "wasm32"))]
    start: std::time::Instant,
}

impl Timer {
    pub(crate) fn now() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            start: std::time::Instant::now(),
        }
    }

    pub(crate) fn elapsed(&self) -> Duration {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.start.elapsed()
        }
        #[cfg(target_arch = "wasm32")]
        {
            Duration::ZERO
        }
    }
}

/// Runs `f` on a `std::thread::scope` pool.
///
/// `wasm32-unknown-unknown` has no threads, so `f` runs once on the calling
/// thread instead. Call sites hoist their scoped body into a closure, so both
/// targets share one code path.
pub(crate) fn par_scope<F>(f: &F)
where
    F: Fn() + Sync,
{
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::scope(|s| {
        let num_threads =
            std::thread::available_parallelism().map_or(crate::DEFAULT_PARALLELISM, |p| p.get());
        for _ in 0..num_threads {
            s.spawn(f);
        }
    });

    #[cfg(target_arch = "wasm32")]
    f();
}

#[derive(Clone, Debug)]
/// An iterator adaptor that allows putting back a single
/// item to the front of the iterator.
///
/// Iterator element type is `I::Item`.
///
/// Code is copied from [itertools](https://docs.rs/itertools/latest/itertools/structs/struct.PutBack.html).
pub struct PutBack<I>
where
    I: Iterator,
{
    top: Option<I::Item>,
    iter: I,
}

pub fn put_back_iterator<I>(iterable: I) -> PutBack<I::IntoIter>
where
    I: IntoIterator,
{
    PutBack {
        top: None,
        iter: iterable.into_iter(),
    }
}
impl<I> PutBack<I>
where
    I: Iterator,
{
    /// Put back a single value to the front of the iterator.
    ///
    /// If a value is already in the put back slot, it is overwritten.
    #[inline]
    pub fn put_back(&mut self, x: I::Item) {
        self.top = Some(x)
    }
}

impl<I> Iterator for PutBack<I>
where
    I: Iterator,
{
    type Item = I::Item;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.top {
            None => self.iter.next(),
            ref mut some => some.take(),
        }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let x = usize::from(self.top.is_some());
        let (low, hi) = self.iter.size_hint();
        (low + x, hi.map(|e| e + x))
    }

    fn count(self) -> usize {
        self.iter.count() + (self.top.is_some() as usize)
    }

    fn last(self) -> Option<Self::Item> {
        self.iter.last().or(self.top)
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        match self.top {
            None => self.iter.nth(n),
            ref mut some => {
                if n == 0 {
                    some.take()
                } else {
                    *some = None;
                    self.iter.nth(n - 1)
                }
            }
        }
    }

    fn all<G>(&mut self, mut f: G) -> bool
    where
        G: FnMut(Self::Item) -> bool,
    {
        if let Some(elt) = self.top.take() {
            if !f(elt) {
                return false;
            }
        }
        self.iter.all(f)
    }

    fn fold<Acc, G>(mut self, init: Acc, mut f: G) -> Acc
    where
        G: FnMut(Acc, Self::Item) -> Acc,
    {
        let mut accum = init;
        if let Some(elt) = self.top.take() {
            accum = f(accum, elt);
        }
        self.iter.fold(accum, f)
    }
}
