use super::super::{config_dir, LibraryIterator, StringError};
use fnv::FnvHashMap;
use libloading::{os::unix::Symbol as RawSymbol, Library, Symbol};
use std::{ffi::CString, fs::read_dir, mem::forget, os::raw::c_char, ptr, slice, str};
use types::Identifier;

/// Either one or the other will be set. Optional status can be conveyed by setting the
/// corresponding field to `NULL`. Libraries importing this structure should check for nullness.
#[repr(C)]
pub(crate) struct RawMethodArguments {
    key_ptr:       *mut c_char,
    key_array_ptr: *mut *mut c_char,
    args_ptr:      *mut *mut c_char,
    key_len:       usize,
    args_len:      usize,
}

pub(crate) enum MethodArguments {
    StringArg(String, Vec<String>),
    Array(Vec<String>, Vec<String>),
    NoArgs,
}

impl From<MethodArguments> for RawMethodArguments {
    fn from(arg: MethodArguments) -> RawMethodArguments {
        match arg {
            MethodArguments::StringArg(string, args) => {
                let args_len = args.len();
                let mut args = args.iter()
                    .map(|x| unsafe {
                        CString::from_vec_unchecked(x.as_bytes().to_owned()).into_raw()
                    })
                    .collect::<Vec<*mut c_char>>();
                args.shrink_to_fit();
                let args_ptr = args.as_mut_ptr();
                forget(args);

                RawMethodArguments {
                    key_ptr: unsafe {
                        CString::from_vec_unchecked(string.as_bytes().to_owned()).into_raw()
                    },
                    key_array_ptr: ptr::null_mut(),
                    args_ptr,
                    key_len: 1,
                    args_len,
                }
            }
            MethodArguments::Array(array, args) => {
                let key_len = array.len();
                let mut key_array = array
                    .iter()
                    .map(|x| unsafe {
                        CString::from_vec_unchecked(x.as_bytes().to_owned()).into_raw()
                    })
                    .collect::<Vec<*mut c_char>>();
                let key_array_ptr = key_array.as_mut_ptr();
                forget(key_array);

                let args_len = args.len();
                let mut args = args.iter()
                    .map(|x| unsafe {
                        CString::from_vec_unchecked(x.as_bytes().to_owned()).into_raw()
                    })
                    .collect::<Vec<*mut c_char>>();
                args.shrink_to_fit();
                let args_ptr = args.as_mut_ptr();
                forget(args);

                RawMethodArguments {
                    key_ptr: ptr::null_mut(),
                    key_array_ptr,
                    args_ptr,
                    key_len,
                    args_len,
                }
            }
            MethodArguments::NoArgs => RawMethodArguments {
                key_ptr:       ptr::null_mut(),
                key_array_ptr: ptr::null_mut(),
                args_ptr:      ptr::null_mut(),
                key_len:       0,
                args_len:      0,
            },
        }
    }
}

/// Contains all dynamically-loaded libraries and their symbols.
///
/// The purpose of the structure is to: A) hold a `Library` handle to the dynamically-loaded
/// plugin to ensure that the plugin remains loaded in memory, and it's contained symbols
/// remain validly-executable; and B) holds a map of functions that may be executed within
/// the namespace.
pub(crate) struct StringMethodPlugins {
    #[allow(dead_code)]
    /// Contains all of the loaded libraries from whence the symbols were
    /// obtained.
    libraries: Vec<Library>,
    /// A map of all the symbols that were collected from the above libraries.
    pub symbols:
        FnvHashMap<Identifier, RawSymbol<unsafe extern "C" fn(RawMethodArguments) -> *mut c_char>>,
}

impl StringMethodPlugins {
    /// Attempts to execute a function within a dynamically-loaded namespace.
    ///
    /// If the function exists, it is executed, and it's return value is then converted into a
    /// proper Rusty type.
    pub(crate) fn execute(
        &self,
        function: &str,
        arguments: MethodArguments,
    ) -> Result<Option<String>, StringError> {
        let func = self.symbols
            .get(function.into())
            .ok_or(StringError::FunctionMissing(function.into()))?;
        unsafe {
            let data = (*func)(RawMethodArguments::from(arguments));
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

    pub(crate) fn load(&mut self, library: Library) -> Result<(), StringError> {
        unsafe {
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
                                unsafe extern "C" fn(RawMethodArguments) -> *mut c_char,
                            > = library
                                .get(symbol.as_slice())
                                .map_err(StringError::SymbolErr)?;

                            // And finally add the name of the function and it's function into the
                            // map.
                            self.symbols.insert(identifier, symbol.into_raw());
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
                    let symbol: Symbol<
                        unsafe extern "C" fn(RawMethodArguments) -> *mut c_char,
                    > = library
                        .get(symbol.as_slice())
                        .map_err(StringError::SymbolErr)?;
                    self.symbols.insert(identifier, symbol.into_raw());
                }
            }

            self.libraries.push(library);
            Ok(())
        }
    }

    pub(crate) fn new() -> StringMethodPlugins {
        StringMethodPlugins {
            libraries: Vec::new(),
            symbols:   FnvHashMap::default(),
        }
    }
}

/// Collects all dynamically-loaded namespaces and their associated symbols all at once.
///
/// This function is meant to be called with `lazy_static` to ensure that there isn't a
/// cost to collecting all this information when the shell never uses it in the first place!
pub(crate) fn collect() -> StringMethodPlugins {
    let mut methods = StringMethodPlugins::new();
    if let Some(mut path) = config_dir() {
        path.push("methods");
        path.push("strings");
        match read_dir(&path).map(LibraryIterator::new) {
            Ok(iterator) => for (_, library) in iterator {
                if let Err(why) = methods.load(library) {
                    eprintln!("ion: string method error: {}", why);
                }
            },
            Err(why) => {
                eprintln!("ion: unable to read methods plugin directory: {}", why);
            }
        }
    }
    methods
}
