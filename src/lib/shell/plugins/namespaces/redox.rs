use fnv::FnvHashMap;

use super::super::StringError;
use types;

pub(crate) struct StringNamespace;

impl StringNamespace {
    pub(crate) fn execute(&self, _function: types::Str) -> Result<Option<types::Str>, StringError> {
        Ok(None)
    }

    pub(crate) fn new() -> Result<StringNamespace, StringError> { Ok(StringNamespace) }
}

pub(crate) fn collect() -> FnvHashMap<types::Str, StringNamespace> {
    eprintln!("ion: Redox doesn't support plugins yet");
    FnvHashMap::default()
}
