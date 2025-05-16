#[macro_export]
macro_rules! string {
    ($s:expr) => {
        String::from($s)
    };
}