use std::process;
use std::io::{self, Write};
use std::mem;
use super::status::*;
use super::Shell;
use super::flags::*;
use super::flow_control::{ElseIf, Function, Statement, collect_loops, collect_if};
use parser::{ForExpression, StatementSplitter, check_statement};
use parser::peg::Pipeline;
use super::assignments::{let_assignment, export_variable};

//use glob::glob;

pub enum Condition {
    Continue,
    Break,
    NoOp
}

pub trait FlowLogic {
    /// Receives a command and attempts to execute the contents.
    fn on_command(&mut self, command_string: &str);

    /// The highest layer of the flow control handling which branches into lower blocks when found.
    fn execute_toplevel<I>(&mut self, iterator: &mut I, statement: Statement) -> Result<(), &'static str>
        where I: Iterator<Item = Statement>;

    /// Executes all of the statements within a while block until a certain condition is met.
    fn execute_while(&mut self, expression: Pipeline, statements: Vec<Statement>);

    /// Executes all of the statements within a for block for each value specified in the range.
    fn execute_for(&mut self, variable: &str, values: &[String], statements: Vec<Statement>);

    /// Conditionally executes branches of statements according to evaluated expressions
    fn execute_if(&mut self, expression: Pipeline, success: Vec<Statement>,
        else_if: Vec<ElseIf>, failure: Vec<Statement>) -> Condition;

    /// Simply executes all supplied statemnts.
    fn execute_statements(&mut self, statements: Vec<Statement>) -> Condition;
}

impl<'a> FlowLogic for Shell<'a> {
    fn on_command(&mut self, command_string: &str) {
        let mut iterator = StatementSplitter::new(command_string).map(check_statement);

        // If the value is set to `0`, this means that we don't need to append to an existing
        // partial statement block in memory, but can read and execute new statements.
        if self.flow_control.level == 0 {
            while let Some(statement) = iterator.next() {
                // Executes all statements that it can, and stores the last remaining partial
                // statement in memory if needed. We can tell if there is a partial statement
                // later if the value of `level` is not set to `0`.
                if let Err(why) = self.execute_toplevel(&mut iterator, statement) {
                    let stderr = io::stderr();
                    let mut stderr = stderr.lock();
                    let _ = writeln!(stderr, "{}", why);
                    self.flow_control.level = 0;
                    self.flow_control.current_if_mode = 0;
                    return
                }
            }
        } else {
            // Appends the newly parsed statements onto the existing statement stored in memory.
            match self.flow_control.current_statement {
                Statement::While{ ref mut statements, .. }
                    | Statement::For { ref mut statements, .. }
                    | Statement::Function { ref mut statements, .. } =>
                {
                    collect_loops(&mut iterator, statements, &mut self.flow_control.level);
                },
                Statement::If { ref mut success, ref mut else_if, ref mut failure, .. } => {
                    self.flow_control.current_if_mode = match collect_if(&mut iterator, success,
                        else_if, failure, &mut self.flow_control.level,
                        self.flow_control.current_if_mode) {
                            Ok(mode) => mode,
                            Err(why) => {
                                let stderr = io::stderr();
                                let mut stderr = stderr.lock();
                                let _ = writeln!(stderr, "{}", why);
                                4
                            }
                        };
                }
                _ => ()
            }

            // If this is true, an error occurred during the if statement
            if self.flow_control.current_if_mode == 4 {
                self.flow_control.level = 0;
                self.flow_control.current_if_mode = 0;
                self.flow_control.current_statement = Statement::Default;
                return
            }

            // If the level is set to 0, it means that the statement in memory is finished
            // and thus is ready for execution.
            if self.flow_control.level == 0 {
                // Replaces the `current_statement` with a `Default` value to avoid the
                // need to clone the value, and clearing it at the same time.
                let mut replacement = Statement::Default;
                mem::swap(&mut self.flow_control.current_statement, &mut replacement);

                match replacement {
                    Statement::Error(number) => self.previous_status = number,
                    Statement::Let { expression } => {
                        self.previous_status = let_assignment(expression, &mut self.variables, &self.directory_stack);
                    },
                    Statement::Export(expression) => {
                        self.previous_status = export_variable(expression, &mut self.variables, &self.directory_stack);
                    }
                    Statement::While { expression, statements } => {
                        self.execute_while(expression, statements);
                    },
                    Statement::For { variable, values, statements } => {
                        self.execute_for(&variable, &values, statements);
                    },
                    Statement::Function { name, args, statements, description } => {
                        self.functions.insert(name.clone(), Function {
                            name:       name,
                            args:       args,
                            statements: statements,
                            description: description,
                        });
                    },
                    Statement::If { expression, success, else_if, failure } => {
                        self.execute_if(expression, success, else_if, failure);
                    }
                    _ => ()
                }

                // Capture any leftover statements.
                while let Some(statement) = iterator.next() {
                    if let Err(why) = self.execute_toplevel(&mut iterator, statement) {
                        let stderr = io::stderr();
                        let mut stderr = stderr.lock();
                        let _ = writeln!(stderr, "{}", why);
                        self.flow_control.level = 0;
                        self.flow_control.current_if_mode = 0;
                        return
                    }
                }
            }
        }
    }

    fn execute_statements(&mut self, mut statements: Vec<Statement>) -> Condition {
        let mut iterator = statements.drain(..);
        while let Some(statement) = iterator.next() {
            match statement {
                Statement::Error(number) => self.previous_status = number,
                Statement::Let { expression } => {
                    self.previous_status = let_assignment(expression, &mut self.variables, &self.directory_stack);
                },
                Statement::Export(expression) => {
                    self.previous_status = export_variable(expression, &mut self.variables, &self.directory_stack);
                }
                Statement::While { expression, mut statements } => {
                    self.flow_control.level += 1;
                    collect_loops(&mut iterator, &mut statements, &mut self.flow_control.level);
                    self.execute_while(expression, statements);
                },
                Statement::For { variable, values, mut statements } => {
                    self.flow_control.level += 1;
                    collect_loops(&mut iterator, &mut statements, &mut self.flow_control.level);
                    self.execute_for(&variable, &values, statements);
                },
                Statement::If { expression, mut success, mut else_if, mut failure } => {
                    self.flow_control.level += 1;
                    if let Err(why) = collect_if(&mut iterator, &mut success, &mut else_if,
                        &mut failure, &mut self.flow_control.level, 0)
                    {
                        let stderr = io::stderr();
                        let mut stderr = stderr.lock();
                        let _ = writeln!(stderr, "{}", why);
                        self.flow_control.level = 0;
                        self.flow_control.current_if_mode = 0;
                        return Condition::Break
                    }

                    match self.execute_if(expression, success, else_if, failure) {
                        Condition::Break    => return Condition::Break,
                        Condition::Continue => return Condition::Continue,
                        Condition::NoOp     => ()
                    }
                },
                Statement::Function { name, args, mut statements, description } => {
                    self.flow_control.level += 1;
                    collect_loops(&mut iterator, &mut statements, &mut self.flow_control.level);
                    self.functions.insert(name.clone(), Function {
                        description: description,
                        name:        name,
                        args:        args,
                        statements:  statements
                    });
                },
                Statement::Pipeline(mut pipeline)  => {
                    self.run_pipeline(&mut pipeline);
                    if self.flags & ERR_EXIT != 0 && self.previous_status != SUCCESS {
                        process::exit(self.previous_status);
                    }
                },
                Statement::Break => { return Condition::Break }
                Statement::Continue => { return Condition::Continue }
                _ => {}
            }
        }
        Condition::NoOp
    }

    fn execute_while(&mut self, expression: Pipeline, statements: Vec<Statement>) {
        while self.run_pipeline(&mut expression.clone()) == Some(SUCCESS) {
            // Cloning is needed so the statement can be re-iterated again if needed.
            if let Condition::Break = self.execute_statements(statements.clone()) {
                break
            }
        }
    }

    fn execute_for(&mut self, variable: &str, values: &[String], statements: Vec<Statement>) {
        /*fn glob_expand(arg: &str) -> Vec<String> {
            let mut expanded = Vec::new();
            if arg.contains(|chr| chr == '?' || chr == '*' || chr == '[') {
                if let Ok(glob) = glob(arg) {
                    for path in glob.filter_map(Result::ok) {
                        expanded.push(path.to_string_lossy().into_owned());
                    }
                }
                expanded
            } else {
                vec![arg.to_owned()]
            }
        }*/

        let ignore_variable = variable == "_";
        match ForExpression::new(values, &self.directory_stack, &self.variables) {
            ForExpression::Multiple(ref values) if ignore_variable => {
                for _ in values.iter()/*.flat_map(|x| glob_expand(&x))*/ {
                    if let Condition::Break = self.execute_statements(statements.clone()) { break }
                }
            },
            ForExpression::Multiple(values) => {
                for value in values.iter()/*.flat_map(|x| glob_expand(&x))*/ {
                    self.variables.set_var(variable, &value);
                    if let Condition::Break = self.execute_statements(statements.clone()) { break }
                }
            },
            ForExpression::Normal(ref values) if ignore_variable => {
                for _ in values.lines()/*.flat_map(glob_expand)*/ {
                    if let Condition::Break = self.execute_statements(statements.clone()) { break }
                }
            },
            ForExpression::Normal(values) => {
                for value in values.lines()/*.flat_map(glob_expand)*/ {
                    self.variables.set_var(variable, &value);
                    if let Condition::Break = self.execute_statements(statements.clone()) { break }
                }
            },
            ForExpression::Range(start, end) if ignore_variable => {
                for _ in start..end {
                    if let Condition::Break = self.execute_statements(statements.clone()) { break }
                }
            }
            ForExpression::Range(start, end) => {
                for value in (start..end).map(|x| x.to_string()) {
                    self.variables.set_var(variable, &value);
                    if let Condition::Break = self.execute_statements(statements.clone()) { break }
                }
            }
        }
    }

    fn execute_if(&mut self, mut expression: Pipeline, success: Vec<Statement>,
        else_if: Vec<ElseIf>, failure: Vec<Statement>) -> Condition
    {
        match self.run_pipeline(&mut expression) {
            Some(SUCCESS) => self.execute_statements(success),
            _             => {
                for mut elseif in else_if {
                    if self.run_pipeline(&mut elseif.expression) == Some(SUCCESS) {
                        return self.execute_statements(elseif.success);
                    }
                }
                self.execute_statements(failure)
            }
        }
    }

    fn execute_toplevel<I>(&mut self, iterator: &mut I, statement: Statement) -> Result<(), &'static str>
        where I: Iterator<Item = Statement>
    {
        match statement {
            Statement::Error(number) => self.previous_status = number,
            // Execute a Let Statement
            Statement::Let { expression } => {
                self.previous_status = let_assignment(expression, &mut self.variables, &self.directory_stack);
            },
            Statement::Export(expression) => {
               self.previous_status = export_variable(expression, &mut self.variables, &self.directory_stack);
            }
            // Collect the statements for the while loop, and if the loop is complete,
            // execute the while loop with the provided expression.
            Statement::While { expression, mut statements } => {
                self.flow_control.level += 1;

                // Collect all of the statements contained within the while block.
                collect_loops(iterator, &mut statements, &mut self.flow_control.level);

                if self.flow_control.level == 0 {
                    // All blocks were read, thus we can immediately execute now
                    self.execute_while(expression, statements);
                } else {
                    // Store the partial `Statement::While` to memory
                    self.flow_control.current_statement = Statement::While {
                        expression: expression,
                        statements: statements,
                    }
                }
            },
            // Collect the statements for the for loop, and if the loop is complete,
            // execute the for loop with the provided expression.
            Statement::For { variable, values, mut statements } => {
                self.flow_control.level += 1;

                // Collect all of the statements contained within the for block.
                collect_loops(iterator, &mut statements, &mut self.flow_control.level);

                if self.flow_control.level == 0 {
                    // All blocks were read, thus we can immediately execute now
                    self.execute_for(&variable, &values, statements);
                } else {
                    // Store the partial `Statement::For` to memory
                    self.flow_control.current_statement = Statement::For {
                        variable:   variable,
                        values:     values,
                        statements: statements,
                    }
                }
            },
            // Collect the statements needed for the `success`, `else_if`, and `failure`
            // conditions; then execute the if statement if it is complete.
            Statement::If { expression, mut success, mut else_if, mut failure } => {
                self.flow_control.level += 1;

                // Collect all of the success and failure statements within the if condition.
                // The `mode` value will let us know whether the collector ended while
                // collecting the success block or the failure block.
                let mode = collect_if(iterator, &mut success, &mut else_if,
                    &mut failure, &mut self.flow_control.level, 0)?;

                if self.flow_control.level == 0 {
                    // All blocks were read, thus we can immediately execute now
                    self.execute_if(expression, success, else_if, failure);
                } else {
                    // Set the mode and partial if statement in memory.
                    self.flow_control.current_if_mode = mode;
                    self.flow_control.current_statement = Statement::If {
                        expression: expression,
                        success:    success,
                        else_if:    else_if,
                        failure:    failure
                    };
                }
            },
            // Collect the statements needed by the function and add the function to the
            // list of functions if it is complete.
            Statement::Function { name, args, mut statements, description } => {
                self.flow_control.level += 1;

                // The same logic that applies to loops, also applies here.
                collect_loops(iterator, &mut statements, &mut self.flow_control.level);

                if self.flow_control.level == 0 {
                    // All blocks were read, thus we can add it to the list
                    self.functions.insert(name.clone(), Function {
                        description: description,
                        name:        name,
                        args:        args,
                        statements:  statements
                    });
                } else {
                    // Store the partial function declaration in memory.
                    self.flow_control.current_statement = Statement::Function {
                        description: description,
                        name:        name,
                        args:        args,
                        statements:  statements
                    }
                }
            },
            // Simply executes a provided pipeline, immediately.
            Statement::Pipeline(mut pipeline)  => {
                self.run_pipeline(&mut pipeline);
                if self.flags & ERR_EXIT != 0 && self.previous_status != SUCCESS {
                    process::exit(self.previous_status);
                }
            },
            // At this level, else and else if keywords are forbidden.
            Statement::ElseIf{..} | Statement::Else => {
                let stderr = io::stderr();
                let mut stderr = stderr.lock();
                let _ = writeln!(stderr, "ion: syntax error: not an if statement");
            },
            // Likewise to else and else if, the end keyword does nothing here.
            Statement::End => {
                let stderr = io::stderr();
                let mut stderr = stderr.lock();
                let _ = writeln!(stderr, "ion: syntax error: no block to end");
            },
            _ => {}
        }
        Ok(())
    }
}
