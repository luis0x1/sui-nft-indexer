use std::str::FromStr;
use anyhow::Error;

#[derive(Debug, Clone)]
pub struct NoneZeroU64(Option<u64>);

impl NoneZeroU64 {
  pub fn default() -> Self {
    Self(None)
  }

  pub fn value(&self) -> u64 {
    self.0.unwrap()
  }

  pub fn is_none(&self) -> bool {
    self.0.is_none()
  }

  pub fn is_some(&self) -> bool {
    self.0.is_some()
  }
}

impl FromStr for NoneZeroU64 {
  type Err = Error;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let value = u64::from_str(s);
    match value {
      Ok(v) => {
        if v == 0 {
          return Ok(Self(None));
        } else {
          return Ok(Self(Some(v)));
        }
      }
      Err(_) => Ok(Self(None)),
    }
  }
}
