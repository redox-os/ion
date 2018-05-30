use super::{
    flags::*,
    flow_control::{collect_cases, collect_if, collect_loops, Case, ElseIf, Function, Statement},
    job_control::JobControl, status::*, Shell,
};
use parser::{
    assignments::{is_array, ReturnValue}, expand_string, parse_and_validate, pipelines::Pipeline,
    ForExpression, StatementSplitter,
};
use shell::assignments::VariableStore;
use std::{
    io::{stdout, Write}, iter, mem,
};
use types::Array;

pub(crate) enum Condition {
    Continue,
    Break,
    NoOp,
    SigInt,
}

pub(crate) trait FlowLogic {
    /// Receives a command and attempts to execute the contents.
    fn on_command(&mut self, command_string: &str);

    /// The highest layer of the flow control handling which branches into lower blocks when
    /// found.
    fn execute_toplevel<I>(
        &mut self,
        iterator: &mut I,
        statement: Statement,
    ) -> Result<(), &'static str>
    where
        I: Iterator<Item = Statement>;

    /// Executes all of the statements within a while block until a certain
    /// condition is met.
    fn execute_while(&mut self, expression: Pipeline, statements: Vec<Statement>) -> Condition;

    /// Executes all of the statements within a for block for each value
    /// specified in the range.
    fn execute_for(
        &mut self,
        variable: &str,
        values: &[String],
        statements: Vec<Statement>,
    ) -> Condition;

    /// Conditionally executes branches of statements according to evaluated
    /// expressions
    fn execute_if(
        &mut self,
        expression: Pipeline,
        success: Vec<Statement>,
        else_if: Vec<ElseIf>,
        failure: Vec<Statement>,
    ) -> Condition;

    /// Simply executes all supplied statemnts.
    fn execute_statements(&mut self, statements: Vec<Statement>) -> Condition;

    /// Executes a single statement
    fn execute_statement<I>(&mut self, iterator: &mut I, statement: Statement) -> Condition
    where
        I: Iterator<Item = Statement>;

    /// Expand an expression and run a branch based on the value of the
    /// expanded expression
    fn execute_match(&mut self, expression: String, cases: Vec<Case>) -> Condition;
}

impl FlowLogic for Shell {
    fn execute_toplevel<I>(
        &mut self,
        iterator: &mut I,
        statement: Statement,
    ) -> Result<(), &'static str>
    where
        I: Iterator<Item = Statement>,
    {
        match statement {
            Statement::Error(number) => self.previous_status = number,
            // Execute a Let Statement
            Statement::Let(action) => {
                self.previous_status = self.local(action);
            }
            Statement::Export(action) => {
                self.previous_status = self.export(action);
            }
            // Collect the statements for the while loop, and if the loop is complete,
            // execute the while loop with the provided expression.
            Statement::While {
                expression,
                mut statements,
            } => {
                self.flow_control.level += 1;

                // Collect all of the statements contained within the while block.
                collect_loops(iterator, &mut statements, &mut self.flow_control.level);

                if self.flow_control.level == 0 {
                    // All blocks were read, thus we can immediately execute now
                    self.execute_while(expression, statements);
                } else {
                    // Store the partial `Statement::While` to memory
                    self.flow_control.current_statement = Statement::While {
                        expression,
                        statements,
                    }
                }
            }
            // Collect the statements for the for loop, and if the loop is complete,
            // execute the for loop with the provided expression.
            Statement::For {
                variable,
                values,
                mut statements,
            } => {
                self.flow_control.level += 1;

                // Collect all of the statements contained within the for block.
                collect_loops(iterator, &mut statements, &mut self.flow_control.level);

                if self.flow_control.level == 0 {
                    // All blocks were read, thus we can immediately execute now
                    self.execute_for(&variable, &values, statements);
                } else {
                    // Store the partial `Statement::For` to memory
                    self.flow_control.current_statement = Statement::For {
                        variable,
                        values,
                        statements,
                    }
                }
            }
            // Collect the statements needed for the `success`, `else_if`, and `failure`
            // conditions; then execute the if statement if it is complete.
            Statement::If {
                expression,
                mut success,
                mut else_if,
                mut failure,
            } => {
                self.flow_control.level += 1;

                // Collect all of the success and failure statements within the if condition.
                // The `mode` value will let us know whether the collector ended while
                // collecting the success block or the failure block.
                let mode = collect_if(
                    iterator,
                    &mut success,
                    &mut else_if,
                    &mut failure,
                    &mut self.flow_control.level,
                    0,
                )?;

                if self.flow_control.level == 0 {
                    // All blocks were read, thus we can immediately execute now
                    self.execute_if(expression, success, else_if, failure);
                } else {
                    // Set the mode and partial if statement in memory.
                    self.flow_control.current_if_mode = mode;
                    self.flow_control.current_statement = Statement::If {
                        expression,
                        success,
                        else_if,
                        failure,
                    };
                }
            }
            // Collect the statements needed by the function and add the function to the
            // list of functions if it is complete.
            Statement::Function {
                name,
                args,
                mut statements,
                description,
            } => {
                self.flow_control.level += 1;

                // The same logic that applies to loops, also applies here.
                collect_loops(iterator, &mut statements, &mut self.flow_control.level);

                if self.flow_control.level == 0 {
                    // All blocks were read, thus we can add it to the list
                    self.functions.insert(
                        name.clone(),
                        Function::new(description, name, args, statements),
                    );
                } else {
                    // Store the partial function declaration in memory.
                    self.flow_control.current_statement = Statement::Function {
                        description,
                        name,
                        args,
                        statements,
                    }
                }
            }
            // Simply executes a provided pipeline, immediately.
            Statement::Pipeline(mut pipeline) => {
                self.run_pipeline(&mut pipeline);
                if self.flags & ERR_EXIT != 0 && self.previous_status != SUCCESS {
                    let status = self.previous_status;
                    self.exit(status);
                }
            }
            Statement::Time(box_statement) => {
                let time = ::std::time::Instant::now();

                if let Err(why) = self.execute_toplevel(iterator, *box_statement) {
                    eprintln!("{}", why);
                    self.flow_control.level = 0;
                    self.flow_control.current_if_mode = 0;
                }
                // Collect timing here so we do not count anything but the execution.
                let duration = time.elapsed();
                let seconds = duration.as_secs();
                let nanoseconds = duration.subsec_nanos();

                if self.flow_control.level == 0 {
                    // A statement was executed, output the time
                    let stdout = stdout();
                    let mut stdout = stdout.lock();
                    let _ = if seconds > 60 {
                        writeln!(
                            stdout,
                            "real    {}m{:02}.{:09}s",
                            seconds / 60,
                            seconds % 60,
                            nanoseconds
                        )
                    } else {
                        writeln!(stdout, "real    {}.{:09}s", seconds, nanoseconds)
                    };
                } else {
                    // A statement wasn't executed , which means that current_statement has been
                    // set to the inner statement. We fix this here.
                    self.flow_control.current_statement =
                        Statement::Time(Box::new(self.flow_control.current_statement.clone()));
                }
            }
            Statement::And(box_statement) => {
                if self.flow_control.level == 0 {
                    match self.previous_status {
                        SUCCESS => {
                            if let Err(why) = self.execute_toplevel(iterator, *box_statement) {
                                eprintln!("{}", why);
                                self.flow_control.level = 0;
                                self.flow_control.current_if_mode = 0;
                            }
                        }
                        _ => (),
                    }
                } else {
                    // A statement wasn't executed , which means that current_statement has been
                    // set to the inner statement. We fix this here.
                    self.flow_control.current_statement =
                        Statement::And(Box::new(self.flow_control.current_statement.clone()));
                }
            }
            Statement::Or(box_statement) => {
                if self.flow_control.level == 0 {
                    match self.previous_status {
                        FAILURE => {
                            if let Err(why) = self.execute_toplevel(iterator, *box_statement) {
                                eprintln!("{}", why);
                                self.flow_control.level = 0;
                                self.flow_control.current_if_mode = 0;
                            }
                        }
                        _ => (),
                    }
                } else {
                    // A statement wasn't executed , which means that current_statement has been
                    // set to the inner statement. We fix this here.
                    self.flow_control.current_statement =
                        Statement::Or(Box::new(self.flow_control.current_statement.clone()));
                }
            }
            Statement::Not(box_statement) => {
                if self.flow_control.level == 0 {
                    if let Err(why) = self.execute_toplevel(iterator, *box_statement) {
                        eprintln!("{}", why);
                        self.flow_control.level = 0;
                        self.flow_control.current_if_mode = 0;
                    }
                    match self.previous_status {
                        FAILURE => self.previous_status = SUCCESS,
                        SUCCESS => self.previous_status = FAILURE,
                        _ => (),
                    }
                    let status = self.previous_status.to_string();
                    self.set_var("?", &status);
                } else {
                    // A statement wasn't executed , which means that current_statement has been
                    // set to the inner statement. We fix this here.
                    self.flow_control.current_statement =
                        Statement::Not(Box::new(self.flow_control.current_statement.clone()));
                }
            }
            // At this level, else and else if keywords are forbidden.
            Statement::ElseIf { .. } | Statement::Else => {
                eprintln!("ion: syntax error: not an if statement");
            }
            // Likewise to else and else if, the end keyword does nothing here.
            Statement::End => {
                eprintln!("ion: syntax error: no block to end");
            }
            // Collect all cases that are being used by a match construct
            Statement::Match {
                expression,
                mut cases,
            } => {
                self.flow_control.level += 1;
                if let Err(why) = collect_cases(iterator, &mut cases, &mut self.flow_control.level)
                {
                    eprintln!("{}", why);
                }
                if self.flow_control.level == 0 {
                    // If all blocks were read we execute the statement
                    self.execute_match(expression, cases);
                } else {
                    // Store the partial function declaration in memory.
                    self.flow_control.current_statement = Statement::Match { expression, cases };
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn execute_if(
        &mut self,
        expression: Pipeline,
        success: Vec<Statement>,
        else_if: Vec<ElseIf>,
        failure: Vec<Statement>,
    ) -> Condition {
        let first_condition = iter::once((expression, success));
        let else_conditions = else_if
            .into_iter()
            .map(|cond| (cond.expression, cond.success));

        for (mut condition, statements) in first_condition.chain(else_conditions) {
            if self.run_pipeline(&mut condition) == Some(SUCCESS) {
                return self.execute_statements(statements);
            }
        }

        self.execute_statements(failure)
    }

    fn execute_for(
        &mut self,
        variable: &str,
        values: &[String],
        statements: Vec<Statement>,
    ) -> Condition {
        let ignore_variable = variable == "_";
        match ForExpression::new(values, self) {
            ForExpression::Multiple(ref values) if ignore_variable => for _ in values.iter() {
                match self.execute_statements(statements.clone()) {
                    Condition::Break => break,
                    Condition::SigInt => return Condition::SigInt,
                    _ => (),
                }
            },
            ForExpression::Multiple(values) => for value in values.iter() {
                self.set_var(variable, &value);
                match self.execute_statements(statements.clone()) {
                    Condition::Break => break,
                    Condition::SigInt => return Condition::SigInt,
                    _ => (),
                }
            },
            ForExpression::Normal(ref values) if ignore_variable => for _ in values.lines() {
                match self.execute_statements(statements.clone()) {
                    Condition::Break => break,
                    Condition::SigInt => return Condition::SigInt,
                    _ => (),
                }
            },
            ForExpression::Normal(values) => for value in values.lines() {
                self.set_var(variable, &value);
                match self.execute_statements(statements.clone()) {
                    Condition::Break => break,
                    Condition::SigInt => return Condition::SigInt,
                    _ => (),
                }
            },
            ForExpression::Range(start, end) if ignore_variable => for _ in start..end {
                match self.execute_statements(statements.clone()) {
                    Condition::Break => break,
                    Condition::SigInt => return Condition::SigInt,
                    _ => (),
                }
            },
            ForExpression::Range(start, end) => for value in (start..end).map(|x| x.to_string()) {
                self.set_var(variable, &value);
                match self.execute_statements(statements.clone()) {
                    Condition::Break => break,
                    Condition::SigInt => return Condition::SigInt,
                    _ => (),
                }
            },
        }
        Condition::NoOp
    }

    fn execute_while(&mut self, expression: Pipeline, statements: Vec<Statement>) -> Condition {
        while self.run_pipeline(&mut expression.clone()) == Some(SUCCESS) {
            // Cloning is needed so the statement can be re-iterated again if needed.
            match self.execute_statements(statements.clone()) {
                Condition::Break => break,
                Condition::SigInt => return Condition::SigInt,
                _ => (),
            }
        }
        Condition::NoOp
    }

    fn execute_statement<I>(&mut self, mut iterator: &mut I, statement: Statement) -> Condition
    where
        I: Iterator<Item = Statement>,
    {
        match statement {
            Statement::Error(number) => self.previous_status = number,
            Statement::Let(action) => {
                self.previous_status = self.local(action);
            }
            Statement::Export(action) => {
                self.previous_status = self.export(action);
            }
            Statement::While {
                expression,
                mut statements,
            } => {
                self.flow_control.level += 1;
                collect_loops(&mut iterator, &mut statements, &mut self.flow_control.level);
                if let Condition::SigInt = self.execute_while(expression, statements) {
                    return Condition::SigInt;
                }
            }
            Statement::For {
                variable,
                values,
                mut statements,
            } => {
                self.flow_control.level += 1;
                collect_loops(&mut iterator, &mut statements, &mut self.flow_control.level);
                if let Condition::SigInt = self.execute_for(&variable, &values, statements) {
                    return Condition::SigInt;
                }
            }
            Statement::If {
                expression,
                mut success,
                mut else_if,
                mut failure,
            } => {
                self.flow_control.level += 1;
                if let Err(why) = collect_if(
                    &mut iterator,
                    &mut success,
                    &mut else_if,
                    &mut failure,
                    &mut self.flow_control.level,
                    0,
                ) {
                    eprintln!("{}", why);
                    self.flow_control.level = 0;
                    self.flow_control.current_if_mode = 0;
                    return Condition::Break;
                }

                match self.execute_if(expression, success, else_if, failure) {
                    Condition::Break => return Condition::Break,
                    Condition::Continue => return Condition::Continue,
                    Condition::NoOp => (),
                    Condition::SigInt => return Condition::SigInt,
                }
            }
            Statement::Function {
                name,
                args,
                mut statements,
                description,
            } => {
                self.flow_control.level += 1;
                collect_loops(&mut iterator, &mut statements, &mut self.flow_control.level);
                self.functions.insert(
                    name.clone(),
                    Function::new(description, name, args, statements),
                );
            }
            Statement::Pipeline(mut pipeline) => {
                self.run_pipeline(&mut pipeline);
                if self.flags & ERR_EXIT != 0 && self.previous_status != SUCCESS {
                    let status = self.previous_status;
                    self.exit(status);
                }
            }
            Statement::Time(box_statement) => {
                let time = ::std::time::Instant::now();

                let condition = self.execute_statement(iterator, *box_statement);

                let duration = time.elapsed();
                let seconds = duration.as_secs();
                let nanoseconds = duration.subsec_nanos();

                let stdout = stdout();
                let mut stdout = stdout.lock();
                let _ = if seconds > 60 {
                    writeln!(
                        stdout,
                        "real    {}m{:02}.{:09}s",
                        seconds / 60,
                        seconds % 60,
                        nanoseconds
                    )
                } else {
                    writeln!(stdout, "real    {}.{:09}s", seconds, nanoseconds)
                };
                match condition {
                    Condition::Break => return Condition::Break,
                    Condition::Continue => return Condition::Continue,
                    Condition::NoOp => (),
                    Condition::SigInt => return Condition::SigInt,
                }
            }
            Statement::And(box_statement) => {
                let condition;
                match self.previous_status {
                    SUCCESS => {
                        condition = self.execute_statement(iterator, *box_statement);
                    }
                    _ => condition = Condition::NoOp,
                }

                match condition {
                    Condition::Break => return Condition::Break,
                    Condition::Continue => return Condition::Continue,
                    Condition::NoOp => (),
                    Condition::SigInt => return Condition::SigInt,
                }
            }
            Statement::Or(box_statement) => {
                let condition;
                match self.previous_status {
                    FAILURE => {
                        condition = self.execute_statement(iterator, *box_statement);
                    }
                    _ => condition = Condition::NoOp,
                }

                match condition {
                    Condition::Break => return Condition::Break,
                    Condition::Continue => return Condition::Continue,
                    Condition::NoOp => (),
                    Condition::SigInt => return Condition::SigInt,
                }
            }
            Statement::Not(box_statement) => {
                let condition = self.execute_statement(iterator, *box_statement);
                match self.previous_status {
                    FAILURE => self.previous_status = SUCCESS,
                    SUCCESS => self.previous_status = FAILURE,
                    _ => (),
                }
                let status = self.previous_status.to_string();
                self.set_var("?", &status);
            }
            Statement::Break => return Condition::Break,
            Statement::Continue => return Condition::Continue,
            Statement::Match {
                expression,
                mut cases,
            } => {
                self.flow_control.level += 1;
                if let Err(why) =
                    collect_cases(&mut iterator, &mut cases, &mut self.flow_control.level)
                {
                    eprintln!("{}", why);
                    self.flow_control.level = 0;
                    self.flow_control.current_if_mode = 0;
                    return Condition::Break;
                }
                match self.execute_match(expression, cases) {
                    Condition::Break => return Condition::Break,
                    Condition::Continue => return Condition::Continue,
                    Condition::NoOp => (),
                    Condition::SigInt => return Condition::SigInt,
                }
            }
            _ => {}
        }
        if let Some(signal) = self.next_signal() {
            if self.handle_signal(signal) {
                self.exit(get_signal_code(signal));
            }
            Condition::SigInt
        } else if self.break_flow {
            self.break_flow = false;
            Condition::SigInt
        } else {
            Condition::NoOp
        }
    }

    fn execute_statements(&mut self, mut statements: Vec<Statement>) -> Condition {
        let mut iterator = statements.drain(..);
        while let Some(statement) = iterator.next() {
            match self.execute_statement(&mut iterator, statement) {
                Condition::NoOp => {}
                cond => return cond,
            }
        }
        Condition::NoOp
    }

    fn execute_match(&mut self, expression: String, cases: Vec<Case>) -> Condition {
        // Logic for determining if the LHS of a match-case construct (the value we are
        // matching against) matches the RHS of a match-case construct (a value
        // in a case statement). For example, checking to see if the value
        // "foo" matches the pattern "bar" would be invoked like so :
        // ```ignore
        // matches("foo", "bar")
        // ```
        fn matches(lhs: &Array, rhs: &Array) -> bool {
            for v in lhs {
                if rhs.contains(&v) {
                    return true;
                }
            }
            false
        }

        let is_array = is_array(&expression);
        let value = expand_string(&expression, self, false);
        let mut condition = Condition::NoOp;
        for case in cases {
            // let pattern_is_array = is_array(&value);
            let pattern = case.value.map(|v| expand_string(&v, self, false));
            match pattern {
                None => {
                    let mut previous_bind = None;
                    if let Some(ref bind) = case.binding {
                        if is_array {
                            previous_bind = self
                                .variables
                                .get_array(bind)
                                .map(|x| ReturnValue::Vector(x.clone()));
                            self.variables.set_array(&bind, value.clone());
                        } else {
                            previous_bind = self.get_var(bind).map(|x| ReturnValue::Str(x));
                            self.set_var(&bind, &value.join(" "));
                        }
                    }

                    if let Some(statement) = case.conditional {
                        self.on_command(&statement);
                        if self.previous_status != SUCCESS {
                            continue;
                        }
                    }

                    condition = self.execute_statements(case.statements);

                    if let Some(ref bind) = case.binding {
                        if let Some(value) = previous_bind {
                            match value {
                                ReturnValue::Str(value) => self.set_var(bind, &value),
                                ReturnValue::Vector(values) => {
                                    self.variables.set_array(bind, values)
                                }
                            }
                        }
                    }

                    break;
                }
                Some(ref v) if matches(v, &value) => {
                    let mut previous_bind = None;
                    if let Some(ref bind) = case.binding {
                        if is_array {
                            previous_bind = self
                                .variables
                                .get_array(bind)
                                .map(|x| ReturnValue::Vector(x.clone()));
                            self.variables.set_array(&bind, value.clone());
                        } else {
                            previous_bind = self.get_var(bind).map(|x| ReturnValue::Str(x));
                            self.set_var(&bind, &value.join(" "));
                        }
                    }

                    if let Some(statement) = case.conditional {
                        self.on_command(&statement);
                        if self.previous_status != SUCCESS {
                            continue;
                        }
                    }

                    condition = self.execute_statements(case.statements);

                    if let Some(ref bind) = case.binding {
                        if let Some(value) = previous_bind {
                            match value {
                                ReturnValue::Str(value) => self.set_var(bind, &value),
                                ReturnValue::Vector(values) => {
                                    self.variables.set_array(bind, values)
                                }
                            }
                        }
                    }

                    break;
                }
                Some(_) => (),
            }
        }
        condition
    }

    fn on_command(&mut self, command_string: &str) {
        self.break_flow = false;
        let mut iterator =
            StatementSplitter::new(command_string).map(parse_and_validate);

        // If the value is set to `0`, this means that we don't need to append to an
        // existing partial statement block in memory, but can read and execute
        // new statements.
        if self.flow_control.level == 0 {
            while let Some(statement) = iterator.next() {
                // Executes all statements that it can, and stores the last remaining partial
                // statement in memory if needed. We can tell if there is a partial statement
                // later if the value of `level` is not set to `0`.
                if let Err(why) = self.execute_toplevel(&mut iterator, statement) {
                    eprintln!("{}", why);
                    self.flow_control.level = 0;
                    self.flow_control.current_if_mode = 0;
                    return;
                }
            }
        } else {
            fn append_new_commands<I: Iterator<Item = Statement>>(
                mut iterator: &mut I,
                current_statement: &mut Statement,
                level: &mut usize,
                current_if_mode: &mut u8,
            ) {
                match current_statement {
                    &mut Statement::While {
                        ref mut statements, ..
                    }
                    | &mut Statement::For {
                        ref mut statements, ..
                    }
                    | &mut Statement::Function {
                        ref mut statements, ..
                    } => {
                        collect_loops(&mut iterator, statements, level);
                    }
                    &mut Statement::If {
                        ref mut success,
                        ref mut else_if,
                        ref mut failure,
                        ..
                    } => {
                        *current_if_mode = match collect_if(
                            &mut iterator,
                            success,
                            else_if,
                            failure,
                            level,
                            *current_if_mode,
                        ) {
                            Ok(mode) => mode,
                            Err(why) => {
                                eprintln!("{}", why);
                                4
                            }
                        };
                    }
                    &mut Statement::Match { ref mut cases, .. } => {
                        if let Err(why) = collect_cases(&mut iterator, cases, level) {
                            eprintln!("{}", why);
                        }
                    }
                    &mut Statement::Time(ref mut box_stmt) => {
                        append_new_commands(iterator, box_stmt.as_mut(), level, current_if_mode);
                    }
                    &mut Statement::And(ref mut box_stmt) => {
                        append_new_commands(iterator, box_stmt.as_mut(), level, current_if_mode);
                    }
                    &mut Statement::Or(ref mut box_stmt) => {
                        append_new_commands(iterator, box_stmt.as_mut(), level, current_if_mode);
                    }
                    &mut Statement::Not(ref mut box_stmt) => {
                        append_new_commands(iterator, box_stmt.as_mut(), level, current_if_mode);
                    }
                    _ => (),
                }
            }

            append_new_commands(
                &mut iterator,
                &mut self.flow_control.current_statement,
                &mut self.flow_control.level,
                &mut self.flow_control.current_if_mode,
            );

            // If this is true, an error occurred during the if statement
            if self.flow_control.current_if_mode == 4 {
                self.flow_control.level = 0;
                self.flow_control.current_if_mode = 0;
                self.flow_control.current_statement = Statement::Default;
                return;
            }

            // If the level is set to 0, it means that the statement in memory is finished
            // and thus is ready for execution.
            if self.flow_control.level == 0 {
                // Replaces the `current_statement` with a `Default` value to avoid the
                // need to clone the value, and clearing it at the same time.
                let mut replacement = Statement::Default;
                mem::swap(&mut self.flow_control.current_statement, &mut replacement);

                fn execute_final(shell: &mut Shell, statement: Statement) -> Condition {
                    match statement {
                        Statement::Error(number) => shell.previous_status = number,
                        Statement::Let(action) => {
                            shell.previous_status = shell.local(action);
                        }
                        Statement::Export(action) => {
                            shell.previous_status = shell.export(action);
                        }
                        Statement::While {
                            expression,
                            statements,
                        } => {
                            if let Condition::SigInt = shell.execute_while(expression, statements) {
                                return Condition::SigInt;
                            }
                        }
                        Statement::For {
                            variable,
                            values,
                            statements,
                        } => {
                            if let Condition::SigInt =
                                shell.execute_for(&variable, &values, statements)
                            {
                                return Condition::SigInt;
                            }
                        }
                        Statement::Function {
                            name,
                            args,
                            statements,
                            description,
                        } => {
                            shell.functions.insert(
                                name.clone(),
                                Function::new(description, name, args, statements),
                            );
                        }
                        Statement::If {
                            expression,
                            success,
                            else_if,
                            failure,
                        } => {
                            shell.execute_if(expression, success, else_if, failure);
                        }
                        Statement::Match { expression, cases } => {
                            shell.execute_match(expression, cases);
                        }
                        Statement::Time(box_stmt) => {
                            let time = ::std::time::Instant::now();

                            let condition = execute_final(shell, *box_stmt);

                            let duration = time.elapsed();
                            let seconds = duration.as_secs();
                            let nanoseconds = duration.subsec_nanos();

                            let stdout = stdout();
                            let mut stdout = stdout.lock();
                            let _ = if seconds > 60 {
                                writeln!(
                                    stdout,
                                    "real    {}m{:02}.{:09}s",
                                    seconds / 60,
                                    seconds % 60,
                                    nanoseconds
                                )
                            } else {
                                writeln!(stdout, "real    {}.{:09}s", seconds, nanoseconds)
                            };
                            return condition;
                        }
                        Statement::And(box_stmt) => match shell.previous_status {
                            SUCCESS => {
                                execute_final(shell, *box_stmt);
                            }
                            _ => (),
                        },
                        Statement::Or(box_stmt) => match shell.previous_status {
                            FAILURE => {
                                execute_final(shell, *box_stmt);
                            }
                            _ => (),
                        },
                        Statement::Not(box_stmt) => {
                            execute_final(shell, *box_stmt);
                            match shell.previous_status {
                                FAILURE => shell.previous_status = SUCCESS,
                                SUCCESS => shell.previous_status = FAILURE,
                                _ => (),
                            }
                            shell
                                .variables
                                .set_var("?", &shell.previous_status.to_string());
                        }
                        _ => (),
                    }
                    Condition::NoOp
                }

                if let Condition::SigInt = execute_final(self, replacement) {
                    return;
                }

                // Capture any leftover statements.
                while let Some(statement) = iterator.next() {
                    if let Err(why) = self.execute_toplevel(&mut iterator, statement) {
                        eprintln!("{}", why);
                        self.flow_control.level = 0;
                        self.flow_control.current_if_mode = 0;
                        return;
                    }
                }
            }
        }
    }
}
