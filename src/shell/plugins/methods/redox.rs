use super::super::StringError;

pub enum MethodArguments {
    StringArg(String, Vec<String>),
    Array(Vec<String>, Vec<String>),
    NoArgs
}

pub struct StringMethodPlugins;

impl StringMethodPlugins {
    pub fn new() -> StringMethodPlugins {
        StringMethodPlugins
    }

    pub fn execute(&self, _function: &str, _arguments: MethodArguments) -> Result<Option<String>, StringError> {
        Ok(None)
    }
}

/// Collects all dynamically-loaded namespaces and their associated symbols all at once.
///
/// This function is meant to be called with `lazy_static` to ensure that there isn't a
/// cost to collecting all this information when the shell never uses it in the first place!
pub fn collect() -> StringMethodPlugins {
    StringMethodPlugins::new()
}
