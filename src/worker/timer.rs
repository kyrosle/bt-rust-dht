use std::{
  collections::BTreeMap,
  pin::Pin,
  task::{Context, Poll},
  time::{Duration, Instant},
};

use futures_util::{Future, Stream};
use tokio::time::{self, Sleep};

/// Timeout information from timer.
///
/// Consider this as a key in BTreeMap.
#[derive(Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub struct Timeout {
  deadline: Instant,
  id: u64,
}

/// A timer manager, accepting the timer entry and insert them to the
/// queue and then waiting for the poll turn.
pub struct Timer<T> {
  /// The id which the timer Entry would get.
  next_id: u64,
  /// The current timer entry.
  current: Option<CurrentTimerEntry<T>>,
  /// The queue keeping the timeout which created by their timer.
  queue: BTreeMap<Timeout, T>,
}

impl<T> Timer<T> {
  pub fn new() -> Self {
    Timer {
      next_id: 0,
      current: None,
      queue: BTreeMap::new(),
    }
  }

  /// Has the timer no scheduled timeouts?
  pub fn is_empty(&self) -> bool {
    self.current.is_none() && self.queue.is_empty()
  }

  /// Schedule the timers and add a new event with a deadline and a value would be returned if ready.
  pub fn schedule_in(&mut self, deadline: Duration, value: T) -> Timeout {
    self.schedule_at(Instant::now() + deadline, value)
  }

  pub fn schedule_at(&mut self, deadline: Instant, value: T) -> Timeout {
    // If the current timeout is later than the new one,
    // push it back into the queue.
    if let Some(current) = &self.current {
      let key = current.key();

      if deadline < key.deadline {
        let CurrentTimerEntry { value, .. } = self.current.take().unwrap();
        self.queue.insert(key, value);
      }
    }

    let id = self.next_id();
    let key = Timeout { deadline, id };
    self.queue.insert(key, value);
    key
  }

  /// Cancel the timeout from the queue or clean the current timer obtaining this timeout.
  pub fn cancel(&mut self, timeout: Timeout) -> bool {
    if let Some(current) = &self.current {
      if current.key() == timeout {
        self.current = None;
        return true;
      }
    }

    self.queue.remove(&timeout).is_some()
  }

  /// Wrapping add 1 to the next_id, and return the id before.
  fn next_id(&mut self) -> u64 {
    let id = self.next_id;
    self.next_id = self.next_id.wrapping_add(1);
    id
  }
}

impl<T: Unpin> Stream for Timer<T> {
  type Item = T;

  fn poll_next(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Option<Self::Item>> {
    loop {
      // poll the current entry sleep event.
      if let Some(current) = &mut self.current {
        match current.sleep.as_mut().poll(cx) {
          Poll::Ready(()) => {
            // self.current -> None
            let CurrentTimerEntry { value, .. } = self.current.take().unwrap();
            // gave out the value.
            return Poll::Ready(Some(value));
          }
          Poll::Pending => return Poll::Pending,
        }
      }
      // TODO: use BTreeMap::pop_first when it becomes stable.
      // check from the queue timeout
      let (key, value) = if let Some(key) = self.queue.keys().next().copied() {
        self.queue.remove_entry(&key).unwrap()
      } else {
        return Poll::Ready(None);
      };

      self.current = Some(CurrentTimerEntry {
        sleep: Box::pin(time::sleep_until(key.deadline.into())),
        value,
        id: key.id,
      })
    }
  }
}

struct CurrentTimerEntry<T> {
  /// The timer sleep event entry.
  sleep: Pin<Box<Sleep>>,
  /// The value would be gave out by the poll event.
  value: T,
  /// Current timer id.
  id: u64,
}

impl<T> CurrentTimerEntry<T> {
  /// Convert timer entry to the Timeout entry, as the key in queue.
  fn key(&self) -> Timeout {
    Timeout {
      deadline: self.sleep.deadline().into_std(),
      id: self.id,
    }
  }
}
