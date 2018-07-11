use super::super::StringError;
use small;

pub(crate) enum MethodArguments {
    StringArg(small::String, Vec<small::String>),
    Array(Vec<small::String>, Vec<small::String>),
    NoArgs,
}

pub(crate) struct StringMethodPlugins;

impl StringMethodPlugins {
    pub(crate) fn execute(
        &self,
        _function: &str,
        _arguments: MethodArguments,
    ) -> Result<Option<small::String>, StringError> {
        Ok(None)
    }

    pub(crate) fn new() -> StringMethodPlugins { StringMethodPlugins }
}

/// Collects all dynamically-loaded namespaces and their associated symbols all at once.
///
/// This function is meant to be called with `lazy_static` to ensure that there isn't a
/// cost to collecting all this information when the shell never uses it in the first place!
pub(crate) fn collect() -> StringMethodPlugins { StringMethodPlugins::new() }
