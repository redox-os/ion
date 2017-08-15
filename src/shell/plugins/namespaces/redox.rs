use fnv::FnvHashMap;

use types::Identifier;
use super::super::StringError;

pub struct StringNamespace;

impl StringNamespace {
    pub fn new() -> Result<StringNamespace, NamespaceError> { Ok(StringNamespace) }

    pub fn execute(&self, function: Identifier) -> Result<Option<String>, NamespaceError> { Ok(None) }
}

pub fn collect() -> FnvHashMap<Identifier, StringNamespace> {
    eprintln!("ion: Redox doesn't support plugins yet");
    FnvHashMap::default()
}
