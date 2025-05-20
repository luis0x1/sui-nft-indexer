use std::sync::{ Arc, LazyLock };

const BLOCKLIST_TYPES: LazyLock<Arc<[&'static str; 3]>> = LazyLock::new(||
  Arc::new([
    "0x2::token::Token<",
    "0x00000000000000000000000000000000000000000000000000000000000000",
    "0x68d22cf8bdbcd11ecba1e094922873e4080d4d11133e2443fddda0bfd11dae20",
  ])
);

pub fn is_in_blocklist(object_type: &str) -> bool {
  BLOCKLIST_TYPES.iter().any(|pat| object_type.starts_with(pat))
}
