pub mod source;
pub mod variables;
pub mod functions;
mod echo;
mod calc;

use self::variables::{alias, drop_alias, drop_variable, export_variable};
use self::functions::fn_;
use self::source::source;
use self::echo::echo;

use fnv::FnvHashMap;
use std::io::{self, Write};
use std::process;
use std::error::Error;

use parser::QuoteTerminator;
use shell::{Shell, FlowLogic, ShellHistory};
use status::*;

/// Structure which represents a Terminal's command.
/// This command structure contains a name, and the code which run the
/// functionnality associated to this one, with zero, one or several argument(s).
/// # Example
/// ```
/// let my_command = Builtin {
///     name: "my_command",
///     help: "Describe what my_command does followed by a newline showing usage",
///     main: box|args: &[String], &mut Shell| -> i32 {
///         println!("Say 'hello' to my command! :-D");
///     }
/// }
/// ```
pub struct Builtin {
    pub name: &'static str,
    pub help: &'static str,
    pub main: Box<Fn(&[String], &mut Shell) -> i32>,
}

impl Builtin {
    /// Return the map from command names to commands
    pub fn map() -> FnvHashMap<&'static str, Self> {
        let mut commands: FnvHashMap<&str, Self> = FnvHashMap::with_capacity_and_hasher(32, Default::default());

        /* Directories */
        commands.insert("cd",
                        Builtin {
                            name: "cd",
                            help: "Change the current directory\n    cd <path>",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                match shell.directory_stack.cd(args, &shell.variables) {
                                    Ok(()) => SUCCESS,
                                    Err(why) => {
                                        let stderr = io::stderr();
                                        let mut stderr = stderr.lock();
                                        let _ = stderr.write_all(why.as_bytes());
                                        FAILURE
                                    }
                                }
                            },
                        });

        commands.insert("dirs",
                        Builtin {
                            name: "dirs",
                            help: "Display the current directory stack",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                shell.directory_stack.dirs(args)
                            },
                        });

        commands.insert("pushd",
                        Builtin {
                            name: "pushd",
                            help: "Push a directory to the stack",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                match shell.directory_stack.pushd(args, &shell.variables) {
                                    Ok(()) => SUCCESS,
                                    Err(why) => {
                                        let stderr = io::stderr();
                                        let mut stderr = stderr.lock();
                                        let _ = stderr.write_all(why.as_bytes());
                                        FAILURE
                                    }
                                }
                            },
                        });

        commands.insert("popd",
                        Builtin {
                            name: "popd",
                            help: "Pop a directory from the stack",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                match shell.directory_stack.popd(args) {
                                    Ok(()) => SUCCESS,
                                    Err(why) => {
                                        let stderr = io::stderr();
                                        let mut stderr = stderr.lock();
                                        let _ = stderr.write_all(why.as_bytes());
                                        FAILURE
                                    }
                                }
                            },
                        });

        /* Aliases */
        commands.insert("alias",
                        Builtin {
                            name: "alias",
                            help: "View, set or unset aliases",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                alias(&mut shell.variables, args)
                            },
                        });

        commands.insert("unalias",
                        Builtin {
                            name: "drop",
                            help: "Delete an alias",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                drop_alias(&mut shell.variables, args)
                            },
                        });

        /* Variables */
        commands.insert("export",
                        Builtin {
                            name: "export",
                            help: "Set an environment variable",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                export_variable(&mut shell.variables, args)
                            }
                        });

        commands.insert("fn",
                        Builtin {
                            name: "fn",
                            help: "Print list of functions",
                            main: box |_: &[String], shell: &mut Shell| -> i32 {
                                fn_(&mut shell.functions)
                            },
                        });

        commands.insert("read",
                        Builtin {
                            name: "read",
                            help: "Read some variables\n    read <variable>",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                shell.variables.read(args)
                            },
                        });

        commands.insert("drop",
                        Builtin {
                            name: "drop",
                            help: "Delete a variable",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                drop_variable(&mut shell.variables, args)
                            },
                        });

        /* Misc */
        commands.insert("eval",
            Builtin {
                name: "eval",
                help: "evaluates the evaluated expression",
                main: box |args: &[String], shell: &mut Shell| -> i32 {
                    let evaluated_command = args[1..].join(" ");
                    let mut buffer = QuoteTerminator::new(evaluated_command);
                    if buffer.check_termination() {
                        shell.on_command(&buffer.consume());
                        shell.previous_status
                    } else {
                        let stderr = io::stderr();
                        let mut stderr = stderr.lock();
                        let _ = writeln!(stderr, "ion: supplied eval expression was not terminted");
                        FAILURE
                    }
                },
            });

        commands.insert("exit",
                Builtin {
                    name: "exit",
                    help: "To exit the curent session",
                    main: box |args: &[String], shell: &mut Shell| -> i32 {
                        process::exit(args.get(1).and_then(|status| status.parse::<i32>().ok())
                            .unwrap_or(shell.previous_status))
                    },
                });

        commands.insert("history",
                        Builtin {
                            name: "history",
                            help: "Display a log of all commands previously executed",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                shell.print_history(args)
                            },
                        });

        commands.insert("source",
                        Builtin {
                            name: "source",
                            help: "Evaluate the file following the command or re-initialize the init file",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                match source(shell, args) {
                                    Ok(()) => SUCCESS,
                                    Err(why) => {
                                        let stderr = io::stderr();
                                        let mut stderr = stderr.lock();
                                        let _ = stderr.write_all(why.as_bytes());
                                        FAILURE
                                    }
                                }

                            },
                        });

        commands.insert("echo",
                        Builtin {
                            name: "echo",
                            help: "Display a line of text",
                            main: box |args: &[String], _: &mut Shell| -> i32 {
                                match echo(args) {
                                    Ok(()) => SUCCESS,
                                    Err(why) => {
                                        let stderr = io::stderr();
                                        let mut stderr = stderr.lock();
                                        let _ = stderr.write_all(why.description().as_bytes());
                                        FAILURE
                                    }
                                }
                            }
                        });

        commands.insert("calc",
                        Builtin {
                            name: "calc",
                            help: "Calculate a mathematical expression",
                            main: box |args: &[String], _: &mut Shell| -> i32 {
                                match calc::calc(&args[1..]) {
                                    Ok(()) => SUCCESS,
                                    Err(why) => {
                                        let stderr = io::stderr();
                                        let mut stderr = stderr.lock();
                                        let _ = writeln!(stderr, "{}", why);
                                        FAILURE
                                    }
                                }
                            }
                        });

        commands.insert("true",
                        Builtin {
                            name: "true",
                            help: "Do nothing, successfully",
                            main: box |_: &[String], _: &mut Shell| -> i32 {
                                SUCCESS
                            },
                        });

        commands.insert("false",
                        Builtin {
                            name: "false",
                            help: "Do nothing, unsuccessfully",
                            main: box |_: &[String], _: &mut Shell| -> i32 {
                                FAILURE
                            },
                        });

        let command_helper: FnvHashMap<&'static str, &'static str> = commands.iter()
                                                                          .map(|(k, v)| {
                                                                              (*k, v.help)
                                                                          })
                                                                          .collect();

        commands.insert("help",
                        Builtin {
                            name: "help",
                            help: "Display helpful information about a given command, or list \
                                   commands if none specified\n    help <command>",
                            main: box move |args: &[String], _: &mut Shell| -> i32 {
                                let stdout = io::stdout();
                                let mut stdout = stdout.lock();
                                if let Some(command) = args.get(1) {
                                    if command_helper.contains_key(command.as_str()) {
                                        if let Some(help) = command_helper.get(command.as_str()) {
                                            let _ = stdout.write_all(help.as_bytes());
                                            let _ = stdout.write_all(b"\n");
                                        }
                                    }
                                    let _ = stdout.write_all(b"Command helper not found [run 'help']...");
                                    let _ = stdout.write_all(b"\n");
                                } else {
                                    let mut commands = command_helper.keys().cloned().collect::<Vec<&str>>();
                                    commands.sort();

                                    let mut buffer: Vec<u8> = Vec::new();
                                    for command in commands {
                                        let _ = writeln!(buffer, "{}", command);
                                    }
                                    let _ = stdout.write_all(&buffer);
                                }
                                SUCCESS
                            },
                        });

        commands
    }
}
