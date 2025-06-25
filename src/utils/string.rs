use std::sync::LazyLock;

#[macro_export]
macro_rules! string {
    ($s:expr) => {
        crate::utils::string::to_string($s)
    };
}

pub fn to_string<T>(value: T) -> String
where T: ToString {
    value.to_string()
}

pub const fn lazy<T>(v: fn() -> T) -> LazyLock<T, fn() -> T> {
    LazyLock::new(v)
}