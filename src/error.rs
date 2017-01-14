use std::io;
use std::fmt;
use std::error;

#[derive(Debug)]
pub enum ProcError {
    Io(io::Error),
    Logic(String),
}

impl fmt::Display for ProcError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ProcError::Io(ref err) => write!(f, "IO error: {}", err),
            ProcError::Logic(ref err) => write!(f, "Logic error: {}", err),
        }
    }
}

impl error::Error for ProcError {
    fn description(&self) -> &str {
        // Both underlying errors already impl `Error`, so we defer to their
        // implementations.
        match *self {
            ProcError::Io(ref err) => err.description(),
            ProcError::Logic(ref message) => &message,
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            // N.B. Both of these implicitly cast `err` from their concrete
            // types (either `&io::Error` or `&num::ParseIntError`)
            // to a trait object `&Error`. This works because both error types
            // implement `Error`.
            ProcError::Io(ref err) => Some(err),
            ProcError::Logic(_) => None,
        }
    }
}

impl From<io::Error> for ProcError {
    fn from(err: io::Error) -> ProcError {
        ProcError::Io(err)
    }
}
