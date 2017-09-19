use fnv::FnvHashMap;

use types::Identifier;
use super::super::StringError;

pub struct StringNamespace;

impl StringNamespace {
    pub fn new() -> Result<StringNamespace, StringError> { Ok(StringNamespace) }

    pub fn execute(&self, _function: Identifier) -> Result<Option<String>, StringError> { Ok(None) }
}

pub fn collect() -> FnvHashMap<Identifier, StringNamespace> {
    eprintln!("ion: Redox doesn't support plugins yet");
    FnvHashMap::default()
}
