use std::error::Error;
use std::fmt;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct StackedError {
    source: Rc<dyn Error>, // Reference to another error
    message: &'static str,
}

impl StackedError {
    // Constructor to create a new WrapedError
    pub fn new(source: Box<dyn Error>, message: &'static str) -> Self {
        StackedError {
            source: source.into(),
            message,
        }
    }

    pub fn from_wraped(wraped: WrapedError, message: &'static str) -> Self {
        StackedError {
            source: wraped.source,
            message,
        }
    }
}

impl fmt::Display for StackedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.source, self.message)
    }
}
impl Error for StackedError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&*self.source) // Return the inner error as the source
    }
}

#[derive(Debug, Clone)]
pub struct WrapedError {
    source: Rc<dyn Error>, // Reference to another error
}

impl WrapedError {
    // Constructor to create a new WrapedError
    pub fn new(source: Box<dyn Error>) -> Self {
        WrapedError {
            source: source.into(),
        }
    }
}

impl fmt::Display for WrapedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.source)
    }
}

impl Error for WrapedError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&*self.source) // Return the inner error as the source
    }
}
