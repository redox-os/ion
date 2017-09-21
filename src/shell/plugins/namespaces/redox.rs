use fnv::FnvHashMap;

use super::super::StringError;
use types::Identifier;

pub(crate) struct StringNamespace;

impl StringNamespace {
    pub(crate) fn new() -> Result<StringNamespace, StringError> { Ok(StringNamespace) }

    pub(crate) fn execute(&self, _function: Identifier) -> Result<Option<String>, StringError> { Ok(None) }
}

pub(crate) fn collect() -> FnvHashMap<Identifier, StringNamespace> {
    eprintln!("ion: Redox doesn't support plugins yet");
    FnvHashMap::default()
}
