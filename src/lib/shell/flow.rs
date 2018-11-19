use itertools::Itertools;
use super::{
    flags::*,
    flow_control::{insert_statement, Case, ElseIf, Function, Statement},
    job_control::JobControl,
    status::*,
    Shell,
};
use parser::{
    assignments::is_array,
    expand_string, parse_and_validate,
    pipelines::{PipeItem, Pipeline},
    ForValueExpression, StatementSplitter,
};
use shell::{assignments::VariableStore, variables::VariableType};
use small;
use std::io::{stdout, Write};
use types;

macro_rules! handle_signal {
    ($signal:expr) => (
        match $signal {
            Condition::Break => break,
            Condition::SigInt => return Condition::SigInt,
            _ => (),
        }
    )
}

#[derive(Debug)]
pub(crate) enum Condition {
    Continue,
    Break,
    NoOp,
    SigInt,
}

pub(crate) trait FlowLogic {
    /// Receives a command and attempts to execute the contents.
    fn on_command(&mut self, command_string: &str);

    /// Executes all of the statements within a while block until a certain
    /// condition is met.
    fn execute_while(
        &mut self,
        expression: Vec<Statement>,
        statements: Vec<Statement>,
    ) -> Condition;

    /// Executes all of the statements within a for block for each value
    /// specified in the range.
    fn execute_for(
        &mut self,
        variables: &[types::Str],
        values: &[small::String],
        statements: Vec<Statement>,
    ) -> Condition;

    /// Conditionally executes branches of statements according to evaluated
    /// expressions
    fn execute_if(
        &mut self,
        expression: Vec<Statement>,
        success: Vec<Statement>,
        else_if: Vec<ElseIf>,
        failure: Vec<Statement>,
    ) -> Condition;

    /// Simply executes all supplied statements.
    fn execute_statements(&mut self, statements: Vec<Statement>) -> Condition;

    /// Executes a single statement
    fn execute_statement(&mut self, statement: Statement) -> Condition;

    /// Expand an expression and run a branch based on the value of the
    /// expanded expression
    fn execute_match(&mut self, expression: small::String, cases: Vec<Case>) -> Condition;
}

impl FlowLogic for Shell {
    fn execute_if(
        &mut self,
        expression: Vec<Statement>,
        success: Vec<Statement>,
        else_if: Vec<ElseIf>,
        failure: Vec<Statement>,
    ) -> Condition {
        // Try execute success branch
        if let Condition::SigInt = self.execute_statements(expression) {
            return Condition::SigInt;
        }
        if self.previous_status == 0 {
            return self.execute_statements(success);
        }

        // Try to execute else_if branches
        let else_if_conditions = else_if
            .into_iter()
            .map(|cond| (cond.expression, cond.success));

        for (condition, statements) in else_if_conditions {
            if let Condition::SigInt = self.execute_statements(condition) {
                return Condition::SigInt;
            }

            if self.previous_status == 0 {
                return self.execute_statements(statements);
            }
        }

        self.execute_statements(failure)
    }

    fn execute_for(
        &mut self,
        variables: &[types::Str],
        values: &[small::String],
        statements: Vec<Statement>,
    ) -> Condition {
        macro_rules! set_vars_then_exec {
            ($chunk:expr, $def:expr) => (
                for (key, value) in variables.iter().zip($chunk.chain(::std::iter::repeat($def))) {
                    if key != "_" {
                        self.set(key, value.clone());
                    }
                }

                handle_signal!(self.execute_statements(statements.clone()));
            )
        }

        let default = ::small::String::new();

        match ForValueExpression::new(values, self) {
            ForValueExpression::Multiple(values) => for chunk in &values.iter().chunks(variables.len()) {
                set_vars_then_exec!(chunk, &default);
            },
            ForValueExpression::Normal(values) => for chunk in &values.lines().chunks(variables.len()) {
                set_vars_then_exec!(chunk, "");
            },
            ForValueExpression::Range(range) => for chunk in &range.chunks(variables.len()) {
                set_vars_then_exec!(chunk, default.clone());
            },
        };

        Condition::NoOp
    }

    fn execute_while(
        &mut self,
        expression: Vec<Statement>,
        statements: Vec<Statement>,
    ) -> Condition {
        loop {
            let expression = {
                self.execute_statements(expression.clone());
                self.previous_status == 0
            };
            if expression {
                // Cloning is needed so the statement can be re-iterated again if needed.
                match self.execute_statements(statements.clone()) {
                    Condition::Break => break,
                    Condition::SigInt => return Condition::SigInt,
                    _ => (),
                }
            } else {
                break;
            }
        }
        Condition::NoOp
    }

    fn execute_statement(&mut self, statement: Statement) -> Condition {
        match statement {
            Statement::Error(number) => {
                self.previous_status = number;
                self.variables.set("?", self.previous_status.to_string());
                self.flow_control.reset();
            }
            Statement::Let(action) => {
                self.previous_status = self.local(action);
                self.variables.set("?", self.previous_status.to_string());
            }
            Statement::Export(action) => {
                self.previous_status = self.export(action);
                self.variables.set("?", self.previous_status.to_string());
            }
            Statement::While {
                expression,
                statements,
            } => {
                if let Condition::SigInt = self.execute_while(expression, statements) {
                    return Condition::SigInt;
                }
            }
            Statement::For {
                variables,
                values,
                statements,
            } => {
                if let Condition::SigInt = self.execute_for(&variables, &values, statements) {
                    return Condition::SigInt;
                }
            }
            Statement::If {
                expression,
                success,
                else_if,
                failure,
                ..
            } => match self.execute_if(expression, success, else_if, failure) {
                Condition::Break => return Condition::Break,
                Condition::Continue => return Condition::Continue,
                Condition::NoOp => (),
                Condition::SigInt => return Condition::SigInt,
            },
            Statement::Function {
                name,
                args,
                statements,
                description,
            } => {
                self.variables.set(
                    &name,
                    Function::new(description, name.clone(), args, statements),
                );
            }
            Statement::Pipeline(pipeline) => match expand_pipeline(&self, pipeline) {
                Ok((mut pipeline, statements)) => {
                    self.run_pipeline(&mut pipeline);
                    if self.flags & ERR_EXIT != 0 && self.previous_status != SUCCESS {
                        let status = self.previous_status;
                        self.exit(status);
                    }
                    if !statements.is_empty() {
                        self.execute_statements(statements);
                    }
                }
                Err(e) => {
                    eprintln!("ion: pipeline expansion error: {}", e);
                    self.previous_status = FAILURE;
                    self.variables.set("?", self.previous_status.to_string());
                    self.flow_control.reset();
                    return Condition::Break;
                }
            },
            Statement::Time(box_statement) => {
                let time = ::std::time::Instant::now();

                let condition = self.execute_statement(*box_statement);

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
                let condition = match self.previous_status {
                    SUCCESS => self.execute_statement(*box_statement),
                    _ => Condition::NoOp,
                };

                match condition {
                    Condition::Break => return Condition::Break,
                    Condition::Continue => return Condition::Continue,
                    Condition::NoOp => (),
                    Condition::SigInt => return Condition::SigInt,
                }
            }
            Statement::Or(box_statement) => {
                let condition = match self.previous_status {
                    FAILURE => self.execute_statement(*box_statement),
                    _ => Condition::NoOp,
                };

                match condition {
                    Condition::Break => return Condition::Break,
                    Condition::Continue => return Condition::Continue,
                    Condition::NoOp => (),
                    Condition::SigInt => return Condition::SigInt,
                }
            }
            Statement::Not(box_statement) => {
                // NOTE: Should the condition be used?
                let _condition = self.execute_statement(*box_statement);
                match self.previous_status {
                    FAILURE => self.previous_status = SUCCESS,
                    SUCCESS => self.previous_status = FAILURE,
                    _ => (),
                }
                let previous_status = self.previous_status.to_string();
                self.set("?", previous_status);
            }
            Statement::Break => return Condition::Break,
            Statement::Continue => return Condition::Continue,
            Statement::Match { expression, cases } => match self.execute_match(expression, cases) {
                Condition::Break => return Condition::Break,
                Condition::Continue => return Condition::Continue,
                Condition::NoOp => (),
                Condition::SigInt => return Condition::SigInt,
            },
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

    fn execute_statements(&mut self, statements: Vec<Statement>) -> Condition {
        self.variables.new_scope(false);

        let mut condition = None;
        for statement in statements {
            match self.execute_statement(statement) {
                Condition::NoOp => {}
                cond => {
                    condition = Some(cond);
                    break;
                }
            }
        }

        self.variables.pop_scope();

        condition.unwrap_or(Condition::NoOp)
    }

    fn execute_match(&mut self, expression: small::String, cases: Vec<Case>) -> Condition {
        // Logic for determining if the LHS of a match-case construct (the value we are
        // matching against) matches the RHS of a match-case construct (a value
        // in a case statement). For example, checking to see if the value
        // "foo" matches the pattern "bar" would be invoked like so :
        // ```ignore
        // matches("foo", "bar")
        // ```
        fn matches(lhs: &types::Array, rhs: &types::Array) -> bool {
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
                                .get::<types::Array>(bind)
                                .map(VariableType::Array);
                            self.variables.set(&bind, value.clone());
                        } else {
                            previous_bind = self
                                .variables
                                .get::<types::Str>(bind)
                                .map(VariableType::Str);
                            self.set(&bind, value.join(" "));
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
                                str_ @ VariableType::Str(_) => {
                                    self.set(bind, str_);
                                }
                                array @ VariableType::Array(_) => {
                                    self.variables.set(bind, array);
                                }
                                map @ VariableType::HashMap(_) => {
                                    self.variables.set(bind, map);
                                }
                                _ => (),
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
                                .get::<types::Array>(bind)
                                .map(VariableType::Array);
                            self.variables.set(&bind, value.clone());
                        } else {
                            previous_bind = self
                                .variables
                                .get::<types::Str>(bind)
                                .map(VariableType::Str);
                            self.set(&bind, value.join(" "));
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
                                str_ @ VariableType::Str(_) => {
                                    self.set(bind, str_);
                                }
                                array @ VariableType::Array(_) => {
                                    self.set(bind, array);
                                }
                                map @ VariableType::HashMap(_) => {
                                    self.set(bind, map);
                                }
                                _ => (),
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
        let iterator = StatementSplitter::new(command_string).map(parse_and_validate);

        // Go through all of the statements and build up the block stack
        // When block is done return statement for execution.
        for statement in iterator {
            match insert_statement(&mut self.flow_control, statement) {
                Err(why) => {
                    eprintln!("{}", why);
                    self.flow_control.reset();
                    return;
                }
                Ok(Some(stm)) => {
                    let _ = self.execute_statement(stm);
                }
                Ok(None) => {}
            }
        }
    }
}

/// Expand a pipeline containing aliases. As aliases can split the pipeline by having logical
/// operators in them, the function returns the first half of the pipeline and the rest of the
/// statements, where the last statement has the other half of the pipeline merged.
fn expand_pipeline(
    shell: &Shell,
    pipeline: Pipeline,
) -> Result<(Pipeline, Vec<Statement>), String> {
    let mut item_iter = pipeline.items.iter();
    let mut items: Vec<PipeItem> = Vec::new();
    let mut statements = Vec::new();

    while let Some(item) = item_iter.next() {
        let possible_alias = shell
            .variables
            .get::<types::Alias>(item.job.command.as_ref());
        if let Some(alias) = possible_alias {
            statements = StatementSplitter::new(alias.0.as_str())
                .map(parse_and_validate)
                .collect();

            // First item in the alias should be a pipeline item, otherwise it cannot
            // be placed into a pipeline!
            let len = statements.len();
            if let Some(Statement::Pipeline(ref mut pline)) = statements.first_mut() {
                // Connect inputs and outputs of alias to pipeline
                if let Some(first) = pline.items.first_mut() {
                    first.inputs = item.inputs.clone();

                    // Add alias arguments to expanded args if there's any.
                    if item.job.args.len() > 1 {
                        for arg in &item.job.args[1..] {
                            first.job.args.push(arg.clone());
                        }
                    }
                }
                if len == 1 {
                    if let Some(last) = pline.items.last_mut() {
                        last.outputs = item.outputs.clone();
                        last.job.kind = item.job.kind;
                    }
                }
                items.append(&mut pline.items);
            } else {
                return Err(format!(
                    "unable to pipe inputs to alias: '{} = {}'",
                    item.job.command.as_str(),
                    alias.0.as_str()
                ));
            }
            statements.remove(0);

            // Handle pipeline being broken half by i.e.: '&&' or '||'
            if ! statements.is_empty() {
                let err = match statements.last_mut().unwrap() {
                    Statement::And(ref mut boxed_stm)
                    | Statement::Or(ref mut boxed_stm)
                    | Statement::Not(ref mut boxed_stm)
                    | Statement::Time(ref mut boxed_stm) => {
                        let stm = &mut **boxed_stm;
                        if let Statement::Pipeline(ref mut pline) = stm {
                            // Set output of alias to be the output of last pipeline.
                            if let Some(last) = pline.items.last_mut() {
                                last.outputs = item.outputs.clone();
                                last.job.kind = item.job.kind;
                            }
                            // Append rest of the pipeline to the last pipeline in the
                            // alias.
                            while let Some(item) = item_iter.next() {
                                pline.items.push(item.clone());
                            }
                            // No error
                            false
                        } else {
                            // Error in expansion
                            true
                        }
                    }
                    _ => true,
                };
                if err {
                    return Err(format!(
                        "unable to pipe outputs of alias: '{} = {}'",
                        item.job.command.as_str(),
                        alias.0.as_str()
                    ));
                }
                break;
            }
        } else {
            items.push(item.clone());
        }
    }
    Ok((Pipeline { items }, statements))
}
