use std::{ collections::VecDeque, sync::Arc };

use tokio::sync::{ Mutex, Notify };

pub struct AsyncVecDeque<T> {
  inner: Arc<Mutex<VecDeque<T>>>,
  notify: Arc<Notify>,
}

impl<T> AsyncVecDeque<T> {
  pub fn new() -> Self {
    Self {
      inner: Arc::new(Mutex::new(VecDeque::new())),
      notify: Arc::new(Notify::new()),
    }
  }

  pub fn with_capacity(capacity: usize) -> Self {
    Self {
      inner: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
      notify: Arc::new(Notify::new()),
    }
  }

  pub async fn push_back(&self, item: T) {
    {
      let mut queue = self.inner.lock().await;
      queue.push_back(item);
    }

    self.notify.notify_one();
  }

  pub async fn push_front(&self, item: T) {
    {
      let mut queue = self.inner.lock().await;
      queue.push_front(item);
    }

    self.notify.notify_one();
  }

  pub async fn take(&self) -> T {
    loop {
      let mut queue = self.inner.lock().await;
      if let Some(item) = queue.pop_front() {
        return item;
      }
      drop(queue);

      self.notify.notified().await;
    }
  }

  pub async fn try_take(&self) -> Option<T> {
    let mut queue = self.inner.lock().await;
    queue.pop_front()
  }

  pub async fn is_empty(&self) -> bool {
    let queue = self.inner.lock().await;
    queue.is_empty()
  }

  pub async fn len(&self) -> usize {
    let queue = self.inner.lock().await;
    queue.len()
  }

  pub async fn clear(&self) {
    let mut queue = self.inner.lock().await;
    queue.clear();
  }
}

impl<T> Clone for AsyncVecDeque<T> {
  fn clone(&self) -> Self {
    Self {
      inner: Arc::clone(&self.inner),
      notify: Arc::clone(&self.notify),
    }
  }
}
