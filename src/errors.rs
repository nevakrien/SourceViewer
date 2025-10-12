use std::any::Any;
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
pub fn downcast_chain_ref<'a, T:Error + 'static>(
    origin: &'a (dyn Error+ 'static),
) -> Option<&'a T> {
    let mut err = origin;
    loop {
        // Try to downcast current error
        if let Some(found) = err.downcast_ref::<T>() {
            return Some(found);
        }

        // Move to next source in the chain
        match err.source() {
            Some(next) => err = next,
            None => return None,
        }
    }
}

use std::io;

#[derive(Debug)]
pub struct PrintError(pub io::Error);

impl fmt::Display for PrintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Print error: {}", self.0)
    }
}

impl Error for PrintError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

/// Macro that behaves like println!, but if writing fails, returns Box<dyn Error>.
#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        use std::io::{self, Write};
        let mut stdout = io::stdout();
        writeln!(stdout, $($arg)*)
            .map_err(|e| Box::new($crate::errors::PrintError(e)) as Box<dyn std::error::Error>)?;
    }};
}
