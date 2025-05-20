use std::time::Duration;

use rand::Rng;
use tokio::time::sleep;

#[derive(Clone, Debug)]
pub struct Backoff {
    base: Duration,
    max: Duration,
}

impl Backoff {
    pub const BACKGROUND: Self = Backoff { base: Duration::from_secs(1), max: Duration::from_secs(60) };
}

impl Backoff {
    pub fn take(&self, attempts: usize) -> BackoffIter {
        BackoffIter { backoff: self.clone(), attempts, next: Duration::ZERO }
    }
}

pub struct BackoffIter {
    backoff: Backoff,
    attempts: usize,
    next: Duration,
}

impl BackoffIter {
    pub async fn wait(&mut self) -> bool {
        match self.next() {
            Some(d) => {
                sleep(d).await;
                true
            }
            None => false,
        }
    }
}

impl Iterator for BackoffIter {
    type Item = Duration;

    fn next(&mut self) -> Option<Self::Item> {
        if self.attempts == 0 {
            return None;
        }
        self.attempts -= 1;

        if self.next.is_zero() {
            self.next = self.backoff.base;
            return Some(Duration::ZERO);
        }

        let current = self.next;
        self.next = std::cmp::min(current.saturating_mul(2), self.backoff.max);
        Some(rand::thread_rng().gen_range((current / 2)..=current))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.attempts, Some(self.attempts))
    }
}
