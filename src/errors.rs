use std::error::Error;
use std::rc::Rc;
use std::fmt;

#[derive(Debug, Clone)]
pub struct WrappedError {
    source: Rc<dyn Error>, // Reference to another error
}

impl WrappedError {
    // Constructor to create a new WrappedError
    pub fn new(source: Box<dyn Error>) -> Self
    {
        WrappedError {
            source: source.into(),
        }
    }
}

impl fmt::Display for WrappedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.source)
    }
}

impl Error for WrappedError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&*self.source) // Return the inner error as the source
    }
}


