use super::super::{LibraryIterator, config_dir};
use fnv::FnvHashMap;
use libloading::{Library, Symbol};
use libloading::os::unix::Symbol as RawSymbol;
use std::ffi::CString;
use std::fmt::{self, Display, Formatter};
use std::fs::read_dir;
use std::io;
use std::slice;
use std::str;
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
            NamespaceError::FunctionMissing(ref func) => write!(f, "{} doesn't exist in namespace", func),
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

pub struct StringNamespace {
    /// Do not remove this field, as it ensures that the library remains loaded.
    #[allow(dead_code)]
    library: Library,
    /// A hash map of symbols collected from the `Library` stored in the `library` field.
    symbols: FnvHashMap<Identifier, RawSymbol<unsafe extern "C" fn() -> NamespaceResult>>,
}

impl StringNamespace {
    pub fn new(library: Library) -> Result<StringNamespace, NamespaceError> {
        unsafe {
            let mut symbols = FnvHashMap::default();
            {
                let index: Symbol<unsafe extern "C" fn() -> *const u8> =
                    library.get(b"index\0").map_err(NamespaceError::SymbolErr)?;
                let symbol_list = index();

                let (mut start, mut counter) = (0, 0usize);
                let symbol_list: &[u8] = {
                    let mut byte = *symbol_list.offset(0);
                    while byte != b'\0' {
                        counter += 1;
                        byte = *symbol_list.offset(counter as isize);
                    }
                    slice::from_raw_parts(symbol_list, counter)
                };

                counter = 0;
                for &byte in symbol_list {
                    if byte == b' ' {
                        if start == counter {
                            start += 1;
                        } else {
                            let slice = &symbol_list[start..counter];
                            let identifier = str::from_utf8(slice).map(Identifier::from).map_err(|_| {
                                NamespaceError::UTF8Function
                            })?;
                            let mut symbol = Vec::new();
                            symbol.reserve_exact(slice.len() + 1);
                            symbol.extend_from_slice(slice);
                            symbol.push(b'\0');
                            let symbol: Symbol<unsafe extern "C" fn() -> NamespaceResult> =
                                library.get(symbol.as_slice()).map_err(
                                    NamespaceError::SymbolErr,
                                )?;
                            symbols.insert(identifier, symbol.into_raw());
                            start = counter + 1;
                        }
                    }
                    counter += 1;
                }

                if counter != start {
                    let slice = &symbol_list[start..];
                    let identifier = str::from_utf8(slice).map(Identifier::from).map_err(|_| {
                        NamespaceError::UTF8Function
                    })?;
                    let mut symbol = Vec::new();
                    symbol.reserve_exact(slice.len() + 1);
                    symbol.extend_from_slice(slice);
                    symbol.push(b'\0');
                    let symbol: Symbol<unsafe extern "C" fn() -> NamespaceResult> =
                        library.get(symbol.as_slice()).map_err(
                            NamespaceError::SymbolErr,
                        )?;
                    symbols.insert(identifier, symbol.into_raw());
                }
            }
            Ok(StringNamespace { library, symbols })
        }
    }

    pub fn execute(&self, function: Identifier) -> Result<Option<String>, NamespaceError> {
        let func = self.symbols.get(&function).ok_or(
            NamespaceError::FunctionMissing(
                function.clone(),
            ),
        )?;
        unsafe {
            match (*func)().into_option() {
                None => Ok(None),
                Some(cstring) => {
                    match cstring.to_str() {
                        Ok(string) => Ok(Some(string.to_owned())),
                        Err(_) => Err(NamespaceError::UTF8Result),
                    }
                }
            }
        }
    }
}

pub fn collect() -> FnvHashMap<Identifier, StringNamespace> {
    let mut hashmap = FnvHashMap::default();
    if let Some(mut path) = config_dir() {
        path.push("namespaces");
        path.push("strings");
        match read_dir(&path).map(LibraryIterator::new) {
            Ok(iterator) => {
                for (identifier, library) in iterator {
                    match StringNamespace::new(library) {
                        Ok(namespace) => {
                            hashmap.insert(identifier, namespace);
                        }
                        Err(why) => {
                            eprintln!("ion: string namespace error: {}", why);
                            continue;
                        }
                    }
                }
            }
            Err(why) => {
                eprintln!("ion: unable to read namespaces plugin directory: {}", why);
            }
        }
    }
    hashmap
}
