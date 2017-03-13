use std::io::{self, Write};
use std::mem;
use status::*;
use super::Shell;

use flow_control::{ElseIf, Function, Statement, collect_loops, collect_if};
use parser::{ForExpression, StatementSplitter};
use parser::peg::{parse, Pipeline};

pub trait FlowLogic {
    fn on_command(&mut self, command_string: &str);
    fn execute_toplevel<I>(&mut self, iterator: &mut I, statement: Statement) -> Result<(), &'static str>
        where I: Iterator<Item = Statement>;
    fn execute_while(&mut self, expression: Pipeline, statements: Vec<Statement>);
    fn execute_for(&mut self, variable: &str, values: &str, statements: Vec<Statement>);
    fn execute_if(&mut self, expression: Pipeline, success: Vec<Statement>,
        else_if: Vec<ElseIf>, failure: Vec<Statement>) -> bool;
    fn execute_statements(&mut self, statements: Vec<Statement>) -> bool;
}

impl FlowLogic for Shell {
    fn on_command(&mut self, command_string: &str) {
        let mut iterator = StatementSplitter::new(command_string).map(parse);

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
                    Statement::While { expression, statements } => {
                        self.execute_while(expression, statements);
                    },
                    Statement::For { variable, values, statements } => {
                        self.execute_for(&variable, &values, statements);
                    },
                    Statement::Function { name, args, statements } => {
                        self.functions.insert(name.clone(), Function {
                            name:       name,
                            args:       args,
                            statements: statements
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

    fn execute_statements(&mut self, mut statements: Vec<Statement>) -> bool {
        let mut iterator = statements.drain(..);
        while let Some(statement) = iterator.next() {
            match statement {
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
                        &mut failure, &mut self.flow_control.level, 0) {
                            let stderr = io::stderr();
                            let mut stderr = stderr.lock();
                            let _ = writeln!(stderr, "{}", why);
                            self.flow_control.level = 0;
                            self.flow_control.current_if_mode = 0;
                            return true
                        }
                    if self.execute_if(expression, success, else_if, failure) {
                        return true
                    }
                },
                Statement::Function { name, args, mut statements } => {
                    self.flow_control.level += 1;
                    collect_loops(&mut iterator, &mut statements, &mut self.flow_control.level);
                    self.functions.insert(name.clone(), Function {
                        name:       name,
                        args:       args,
                        statements: statements
                    });
                },
                Statement::Pipelines(mut pipelines) => {
                    for mut pipeline in pipelines.drain(..) {
                        self.run_pipeline(&mut pipeline, false);
                    }
                },
                Statement::Break => {
                    return true
                }
                _ => {}
            }
        }
        false
    }

    fn execute_while(&mut self, mut expression: Pipeline, statements: Vec<Statement>) {
        while self.run_pipeline(&mut expression, false) == Some(SUCCESS) {
            // Cloning is needed so the statement can be re-iterated again if needed.
            if self.execute_statements(statements.clone()) {
                break
            }
        }
    }

    fn execute_for(&mut self, variable: &str, values: &str, statements: Vec<Statement>) {
        match ForExpression::new(values, &self.directory_stack, &self.variables) {
            ForExpression::Normal(expression) => {
                for value in expression.split_whitespace() {
                    if value != "_" { self.variables.set_var(variable, value); }
                    if self.execute_statements(statements.clone()) {
                        break
                    }
                }
            },
            ForExpression::Range(start, end) => {
                for value in (start..end).map(|x| x.to_string()) {
                    if value != "_" { self.variables.set_var(variable, &value); }
                    if self.execute_statements(statements.clone()) {
                        break
                    }
                }
            }
        }
    }

    fn execute_if(&mut self, mut expression: Pipeline, success: Vec<Statement>,
        mut else_if: Vec<ElseIf>, failure: Vec<Statement>) -> bool
    {
        match self.run_pipeline(&mut expression, false) {
            Some(SUCCESS) => self.execute_statements(success),
            _             => {
                for mut elseif in else_if.drain(..) {
                    if self.run_pipeline(&mut elseif.expression, false) == Some(SUCCESS) {
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

                // Collect all of the statements contained within the while block.
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
            Statement::Function { name, args, mut statements } => {
                self.flow_control.level += 1;

                // The same logic that applies to loops, also applies here.
                collect_loops(iterator, &mut statements, &mut self.flow_control.level);

                if self.flow_control.level == 0 {
                    // All blocks were read, thus we can add it to the list
                    self.functions.insert(name.clone(), Function {
                        name:       name,
                        args:       args,
                        statements: statements
                    });
                } else {
                    // Store the partial function declaration in memory.
                    self.flow_control.current_statement = Statement::Function {
                        name:       name,
                        args:       args,
                        statements: statements
                    }
                }
            },
            // Simply executes a provide pipeline, immediately.
            Statement::Pipelines(mut pipelines) => {
                // Immediately execute the command as it has no dependents.
                for mut pipeline in pipelines.drain(..) {
                    let _ = self.run_pipeline(&mut pipeline, false);
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
