use super::super::{config_dir, LibraryIterator, StringError};
use fnv::FnvHashMap;
use libloading::{os::unix::Symbol as RawSymbol, Library, Symbol};
use std::{ffi::CString, fs::read_dir, os::raw::c_char, slice, str};
use types::Identifier;

/// A dynamically-loaded string namespace from an external library.
///
/// The purpose of the structure is to: A) hold a `Library` handle to the dynamically-loaded
/// plugin to ensure that the plugin remains loaded in memory, and it's contained symbols
/// remain validly-executable; and B) holds a map of functions that may be executed within
/// the namespace.
pub(crate) struct StringNamespace {
    /// This field, although never used directly, is required to exist in order to ensure
    /// that each element in the `symbols` field remains relevant. When Rust can support
    /// self-referencing lifetimes, we won't need to hold raw symbols anymore.
    #[allow(dead_code)]
    library: Library,
    /// A hash map of symbols collected from the `Library` stored in the `library` field.
    /// These are considered raw because they have their lifetimes erased.
    symbols: FnvHashMap<Identifier, RawSymbol<unsafe extern "C" fn() -> *mut c_char>>,
}

impl StringNamespace {
    /// Attempts to execute a function within a dynamically-loaded namespace.
    ///
    /// If the function exists, it is executed, and it's return value is then converted into a
    /// proper Rusty type.
    pub(crate) fn execute(&self, function: Identifier) -> Result<Option<String>, StringError> {
        let func = self.symbols
            .get(&function)
            .ok_or(StringError::FunctionMissing(function.clone()))?;
        unsafe {
            let data = (*func)();
            if data.is_null() {
                Ok(None)
            } else {
                match CString::from_raw(data as *mut c_char).to_str() {
                    Ok(string) => Ok(Some(string.to_owned())),
                    Err(_) => Err(StringError::UTF8Result),
                }
            }
        }
    }

    pub(crate) fn new(library: Library) -> Result<StringNamespace, StringError> {
        unsafe {
            let mut symbols = FnvHashMap::default();
            {
                // The `index` function contains a list of functions provided by the library.
                let index: Symbol<unsafe extern "C" fn() -> *const u8> =
                    library.get(b"index\0").map_err(StringError::SymbolErr)?;
                let symbol_list = index();

                // Yet we need to convert the raw stream of binary into a native slice if we
                // want to properly reason about the contents of said aforementioned stream.
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

                // Each function symbol is delimited by spaces, so this will slice our
                // newly-created byte slice on each space, and then attempt to load and
                // store that symbol for future use.
                for &byte in symbol_list {
                    if byte == b' ' {
                        if start == counter {
                            start += 1;
                        } else {
                            // Grab a slice and ensure that the name of the function is UTF-8.
                            let slice = &symbol_list[start..counter];
                            let identifier = str::from_utf8(slice)
                                .map(Identifier::from)
                                .map_err(|_| StringError::UTF8Function)?;

                            // To obtain the symbol, we need to create a new `\0`-ended byte array.
                            // TODO: There's no need to use a vector here. An array will do fine.
                            let mut symbol = Vec::new();
                            symbol.reserve_exact(slice.len() + 1);
                            symbol.extend_from_slice(slice);
                            symbol.push(b'\0');

                            // Then attempt to load that symbol from the dynamic library.
                            let symbol: Symbol<
                                unsafe extern "C" fn() -> *mut c_char,
                            > = library
                                .get(symbol.as_slice())
                                .map_err(StringError::SymbolErr)?;

                            // And finally add the name of the function and it's function into the
                            // map.
                            symbols.insert(identifier, symbol.into_raw());
                            start = counter + 1;
                        }
                    }
                    counter += 1;
                }

                // Identical to the logic in the loop above. Handles any unparsed stragglers
                // that have been left over.
                if counter != start {
                    let slice = &symbol_list[start..];
                    let identifier = str::from_utf8(slice)
                        .map(Identifier::from)
                        .map_err(|_| StringError::UTF8Function)?;
                    let mut symbol = Vec::new();
                    symbol.reserve_exact(slice.len() + 1);
                    symbol.extend_from_slice(slice);
                    symbol.push(b'\0');
                    let symbol: Symbol<unsafe extern "C" fn() -> *mut c_char> = library
                        .get(symbol.as_slice())
                        .map_err(StringError::SymbolErr)?;
                    symbols.insert(identifier, symbol.into_raw());
                }
            }

            Ok(StringNamespace { library, symbols })
        }
    }
}

/// Collects all dynamically-loaded namespaces and their associated symbols all at once.
///
/// This function is meant to be called with `lazy_static` to ensure that there isn't a
/// cost to collecting all this information when the shell never uses it in the first place!
pub(crate) fn collect() -> FnvHashMap<Identifier, StringNamespace> {
    let mut hashmap = FnvHashMap::default();
    if let Some(mut path) = config_dir() {
        path.push("namespaces");
        path.push("strings");
        match read_dir(&path).map(LibraryIterator::new) {
            Ok(iterator) => for (identifier, library) in iterator {
                match StringNamespace::new(library) {
                    Ok(namespace) => {
                        hashmap.insert(identifier, namespace);
                    }
                    Err(why) => {
                        eprintln!("ion: string namespace error: {}", why);
                        continue;
                    }
                }
            },
            Err(why) => {
                eprintln!("ion: unable to read namespaces plugin directory: {}", why);
            }
        }
    }
    hashmap
}
