use types::Identifier;
use super::NamespaceError;

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
