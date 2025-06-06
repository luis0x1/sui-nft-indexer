use std::{ future::Future, thread, time::Duration };

use anyhow::Result;

pub struct RetryOptions {
  count: Option<u8>,
  delay: Option<u32>, //miliseconds
}

pub async fn retry<T, R, S>(callback: T, count: u8) -> Result<S>
  where T: Fn() -> R, R: Future<Output = Result<S>>
{
  let mut index = 0;
  loop {
    let callback_res = callback().await;
    let value = match callback_res {
      Ok(res) => Ok(res),
      Err(err) => {
        index += 1;
        if index < count {
          thread::sleep(Duration::from_secs(index as u64));
          continue;
        }

        Err(err)
      }
    };
    return value;
  }
}

pub async fn vretry<T, R, S, Params>(callback: T, count: u8, params: &Params) -> Result<S>
  where T: Fn(&Params) -> R, R: Future<Output = Result<S>>
{
  let mut index = 0;
  loop {
    let callback_res = callback(params).await;
    let value = match callback_res {
      Ok(res) => Ok(res),
      Err(err) => {
        index += 1;
        if index < count {
          thread::sleep(Duration::from_secs(index as u64));
          continue;
        }

        Err(err)
      }
    };
    return value;
  }
}

pub async fn retry_with_options<T, R, S>(callback: T, options: RetryOptions) -> Result<S>
  where T: Fn() -> R, R: Future<Output = Result<S>>
{
  let count = options.count.unwrap_or(1);
  let mut delay = options.delay.unwrap_or(1000);
  let mut index: u8 = 0;

  loop {
    let callback_res = callback().await;
    let value = match callback_res {
      Ok(res) => Ok(res),
      Err(err) => {
        index += 1;
        delay *= index as u32;

        if index < count {
          thread::sleep(Duration::from_millis(delay as u64));
          continue;
        }

        Err(err)
      }
    };

    return value;
  }
}

impl RetryOptions {
  pub fn default() -> Self {
    Self { count: Option::Some(3), delay: Option::Some(1000) }
  }

  pub fn with_count(self, count: u8) -> Self {
    Self { count: Option::Some(count), delay: self.delay }
  }

  pub fn with_delay(self, delay: u32) -> Self {
    Self { count: self.count, delay: Option::Some(delay) }
  }
}
