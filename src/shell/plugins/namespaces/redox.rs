use std::ffi::CString;
use std::fmt::{self, Display, Formatter};
use std::io;
use types::Identifier;

#[derive(Debug)]
pub enum NamespaceError {
    SymbolErr(io::Error),
    UTF8Function,
    UTF8Result,
    FunctionMissing(Identifier),
}

impl Display for NamespaceError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            NamespaceError::SymbolErr(ref error) => write!(f, "symbol error: {}", error),
            NamespaceError::UTF8Function => write!(f, "function has invalid UTF-8 name"),
            NamespaceError::UTF8Result => write!(f, "result is not valid UTF-8"),
            NamespaceError::FunctionMissing(func) => write!(f, "{} doesn't exist in namespace", func),
        }
    }
}

#[repr(C)]
#[derive(Debug)]
struct NamespaceResult {
    exists: bool,
    data: *mut i8,
}

impl NamespaceResult {
    fn into_option(self) -> Option<CString> {
        if self.exists { Some(unsafe { CString::from_raw(self.data) }) } else { None }
    }
}

pub struct StringNamespace {}

impl StringNamespace {
    pub fn new() -> Result<StringNamespace, NamespaceError> { Err(NamespaceError::FunctionMissing) }

    pub fn execute(&self, function: Identifier) -> Result<Option<String>, NamespaceError> { Ok(None) }
}

pub fn collect() -> FnvHashMap<Identifier, StringNamespace> {
    eprintln!("ion: Redox doesn't support plugins yet");
    let mut hashmap = FnvHashMap::default();
    hashmap
}
