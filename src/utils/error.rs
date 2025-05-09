use std::{error::Error, fmt::Display};


#[derive(Debug)]
pub struct AppError {
    message: String,
}

impl AppError {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }

    pub fn message(&self) -> String {
        self.message.clone()
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

pub fn unwrap<T, E: std::error::Error>(result: Result<T, E>) -> Result<T, AppError> {
    match result {
        Ok(v) => Ok(v),
        Err(e) => Err(AppError::new(&e.to_string())),
    }
}
