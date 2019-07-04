use super::{
    flow_control::{Block, Case, ElseIf, Function, IfMode, Statement},
    pipe_exec::PipelineError,
    signals, Shell,
};
use crate::{
    assignments::is_array,
    expansion::{
        pipelines::{PipeItem, Pipeline},
        Expander, ForValueExpression,
    },
    parser::{parse_and_validate, StatementSplitter, Terminator},
    shell::{IonError, Job, Value},
    types,
};
use err_derive::Error;
use itertools::Itertools;
use nix::unistd::Pid;
use std::rc::Rc;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Condition {
    Continue,
    Break,
    NoOp,
}

type Result = std::result::Result<Condition, IonError>;

/// The block order was invalid
#[derive(Debug, Error, PartialEq, Eq, Hash)]
pub enum BlockError {
    /// A case block was found outside a match block
    #[error(display = "Case found outside of Match block")]
    LoneCase,
    /// A statement was found outside a case block in a match
    #[error(display = "statement found outside of Case block in Match")]
    StatementOutsideMatch,

    /// End found, but there is no block to close
    #[error(display = "End found but no block to close")]
    UnmatchedEnd,
    /// An elseif was found without a corresponding if block
    #[error(display = "found ElseIf without If block")]
    LoneElseIf,
    /// A else block was found without its corresponding if block
    #[error(display = "found Else without If block")]
    LoneElse,
    /// Multiple else were found for the same if
    #[error(display = "Else block already exists")]
    MultipleElse,
    /// Elseif block found after else
    #[error(display = "ElseIf found after Else")]
    ElseWrongOrder,

    /// Found a break outside a loop
    #[error(display = "found Break without loop body")]
    UnmatchedBreak,
    /// Found a continue outside a loop
    #[error(display = "found Continue without loop body")]
    UnmatchedContinue,

    /// Unclosed block
    #[error(display = "expected end block for `{}`", _0)]
    UnclosedBlock(String),
}

impl<'a> Shell<'a> {
    fn insert_into_block(
        block: &mut Block<'a>,
        statement: Statement<'a>,
    ) -> std::result::Result<(), BlockError> {
        let block = match block.last_mut().expect("Should not insert statement if stack is empty!")
        {
            Statement::Time(inner) => inner,
            top_block => top_block,
        };

        match block {
            Statement::Function { ref mut statements, .. }
            | Statement::For { ref mut statements, .. }
            | Statement::While { ref mut statements, .. } => statements.push(statement),
            Statement::Match { ref mut cases, .. } => {
                if let Statement::Case(case) = statement {
                    cases.push(case)
                } else {
                    return Err(BlockError::StatementOutsideMatch);
                }
            }
            Statement::Case(ref mut case) => case.statements.push(statement),
            Statement::If {
                ref mut success,
                ref mut else_if,
                ref mut failure,
                ref mut mode,
                ..
            } => match statement {
                Statement::ElseIf(eif) => {
                    if *mode == IfMode::Else {
                        return Err(BlockError::ElseWrongOrder);
                    } else {
                        *mode = IfMode::ElseIf;
                        else_if.push(eif);
                    }
                }
                Statement::Else => {
                    if *mode == IfMode::Else {
                        return Err(BlockError::MultipleElse);
                    } else {
                        *mode = IfMode::Else;
                    }
                }
                _ => match mode {
                    IfMode::Success => success.push(statement),
                    IfMode::ElseIf => else_if.last_mut().unwrap().success.push(statement),
                    IfMode::Else => failure.push(statement),
                },
            },
            _ => unreachable!("Not block-like statement pushed to stack!"),
        }
        Ok(())
    }

    fn insert_statement(
        block: &mut Block<'a>,
        statement: Statement<'a>,
    ) -> std::result::Result<Option<Statement<'a>>, BlockError> {
        match statement {
            // Push new block to stack
            Statement::For { .. }
            | Statement::While { .. }
            | Statement::Match { .. }
            | Statement::If { .. }
            | Statement::Function { .. } => {
                block.push(statement);
                Ok(None)
            }
            // Case is special as it should pop back previous Case
            Statement::Case(_) => {
                match block.last() {
                    Some(Statement::Case(_)) => {
                        let case = block.pop().unwrap();
                        let _ = Self::insert_into_block(block, case);
                    }
                    Some(Statement::Match { .. }) => (),
                    _ => return Err(BlockError::LoneCase),
                }

                block.push(statement);
                Ok(None)
            }
            Statement::End => {
                match block.len() {
                    0 => Err(BlockError::UnmatchedEnd),
                    // Ready to return the complete block
                    1 => Ok(block.pop()),
                    // Merge back the top block into the previous one
                    _ => {
                        let last_statement = block.pop().unwrap();
                        if let Statement::Case(_) = last_statement {
                            Self::insert_into_block(block, last_statement)?;
                            // Merge last Case back and pop off Match too
                            let match_stm = block.pop().unwrap();
                            if block.is_empty() {
                                Ok(Some(match_stm))
                            } else {
                                Self::insert_into_block(block, match_stm)?;
                                Ok(None)
                            }
                        } else {
                            Self::insert_into_block(block, last_statement)?;
                            Ok(None)
                        }
                    }
                }
            }
            Statement::And(_) | Statement::Or(_) if !block.is_empty() => {
                let pushed = match block.last_mut().unwrap() {
                    Statement::If {
                        ref mut expression,
                        ref mode,
                        ref success,
                        ref mut else_if,
                        ..
                    } => match mode {
                        IfMode::Success if success.is_empty() => {
                            // Insert into If expression if there's no previous statement.
                            expression.push(statement.clone());
                            true
                        }
                        IfMode::ElseIf => {
                            // Try to insert into last ElseIf expression if there's no previous
                            // statement.
                            let eif = else_if.last_mut().expect("Missmatch in 'If' mode!");
                            if eif.success.is_empty() {
                                eif.expression.push(statement.clone());
                                true
                            } else {
                                false
                            }
                        }
                        _ => false,
                    },
                    Statement::While { ref mut expression, ref statements } => {
                        if statements.is_empty() {
                            expression.push(statement.clone());
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                };
                if !pushed {
                    Self::insert_into_block(block, statement)?;
                }

                Ok(None)
            }
            Statement::Time(inner) => {
                if inner.is_block() {
                    block.push(Statement::Time(inner));
                    Ok(None)
                } else {
                    Ok(Some(Statement::Time(inner)))
                }
            }
            _ if block.is_empty() => {
                // Filter out toplevel statements that should produce an error
                // otherwise return the statement for immediat execution
                match statement {
                    Statement::ElseIf(_) => Err(BlockError::LoneElseIf),
                    Statement::Else => Err(BlockError::LoneElse),
                    Statement::Break => Err(BlockError::UnmatchedBreak),
                    Statement::Continue => Err(BlockError::UnmatchedContinue),
                    // Toplevel statement, return to execute immediately
                    _ => Ok(Some(statement)),
                }
            }
            _ => {
                Self::insert_into_block(block, statement)?;
                Ok(None)
            }
        }
    }

    /// Conditionally executes branches of statements according to evaluated
    /// expressions
    fn execute_if(
        &mut self,
        expression: &[Statement<'a>],
        success: &[Statement<'a>],
        else_if: &[ElseIf<'a>],
        failure: &[Statement<'a>],
    ) -> Result {
        // Try execute success branch
        self.execute_statements(expression)?;
        if self.previous_status.is_success() {
            return self.execute_statements(success);
        }

        // Try to execute else_if branches
        for ElseIf { expression, success } in else_if {
            self.execute_statements(expression)?;

            if self.previous_status.is_success() {
                return self.execute_statements(success);
            }
        }

        self.execute_statements(failure)
    }

    /// Executes all of the statements within a for block for each value
    /// specified in the range.
    fn execute_for(
        &mut self,
        variables: &[types::Str],
        values: &[types::Str],
        statements: &[Statement<'a>],
    ) -> Result {
        macro_rules! set_vars_then_exec {
            ($chunk:expr, $def:expr) => {
                for (key, value) in variables.iter().zip($chunk.chain(::std::iter::repeat($def))) {
                    if key != "_" {
                        self.variables_mut().set(key, value.clone());
                    }
                }

                if self.execute_statements(statements)? == Condition::Break {
                    break;
                }
            };
        }

        let default = types::Str::new();

        match ForValueExpression::new(values, self)? {
            ForValueExpression::Multiple(values) => {
                for chunk in &values.iter().chunks(variables.len()) {
                    set_vars_then_exec!(chunk, &default);
                }
            }
            ForValueExpression::Normal(value) => {
                if &variables[0] != "_" {
                    self.variables_mut().set(&variables[0], value);
                }

                self.execute_statements(statements)?;
            }
            ForValueExpression::Range(range) => {
                for chunk in &range.chunks(variables.len()) {
                    set_vars_then_exec!(chunk, default.clone());
                }
            }
        };

        Ok(Condition::NoOp)
    }

    /// Executes all of the statements within a while block until a certain
    /// condition is met.
    fn execute_while(
        &mut self,
        expression: &[Statement<'a>],
        statements: &[Statement<'a>],
    ) -> Result {
        loop {
            self.execute_statements(expression)?;
            if !self.previous_status.is_success() {
                return Ok(Condition::NoOp);
            }

            // Cloning is needed so the statement can be re-iterated again if needed.
            if self.execute_statements(statements)? == Condition::Break {
                return Ok(Condition::NoOp);
            }
        }
    }

    /// Executes a single statement
    pub fn execute_statement(&mut self, statement: &Statement<'a>) -> Result {
        match statement {
            Statement::Let(action) => {
                self.previous_status = self.local(action);
                self.variables.set("?", self.previous_status);
            }
            Statement::Export(action) => {
                self.previous_status = self.export(action);
                self.variables.set("?", self.previous_status);
            }
            Statement::While { expression, statements } => {
                self.execute_while(expression, statements)?;
            }
            Statement::For { variables, values, statements } => {
                self.execute_for(variables, values, statements)?;
            }
            Statement::If { expression, success, else_if, failure, .. } => {
                let condition = self.execute_if(expression, success, else_if, failure)?;

                if condition != Condition::NoOp {
                    return Ok(condition);
                }
            }
            Statement::Function { name, args, statements, description } => {
                self.variables.set(
                    name,
                    Value::Function(Rc::new(Function::new(
                        description.clone(),
                        name.clone(),
                        args.to_vec(),
                        statements.to_vec(),
                    ))),
                );
            }
            Statement::Pipeline(pipeline) => {
                let (pipeline, statements) = expand_pipeline(self, pipeline)?;
                if !pipeline.items.is_empty() {
                    let status = self.run_pipeline(&pipeline)?;

                    // Retrieve the exit_status and set the $? variable and
                    // history.previous_status
                    self.variables_mut().set("?", status);
                    self.previous_status = status;
                }
                if !statements.is_empty() {
                    self.execute_statements(&statements)?;
                }
            }
            Statement::Time(box_statement) => {
                let time = std::time::Instant::now();

                let condition = self.execute_statement(box_statement)?;

                let duration = time.elapsed();
                let seconds = duration.as_secs();
                let nanoseconds = duration.subsec_nanos();

                if seconds > 60 {
                    println!("real    {}m{:02}.{:09}s", seconds / 60, seconds % 60, nanoseconds);
                } else {
                    println!("real    {}.{:09}s", seconds, nanoseconds);
                }
                if condition != Condition::NoOp {
                    return Ok(condition);
                }
            }
            Statement::And(box_statement) => {
                let condition = if self.previous_status.is_success() {
                    self.execute_statement(box_statement)?
                } else {
                    Condition::NoOp
                };

                if condition != Condition::NoOp {
                    return Ok(condition);
                }
            }
            Statement::Or(box_statement) => {
                let condition = if self.previous_status.is_success() {
                    Condition::NoOp
                } else {
                    self.execute_statement(box_statement)?
                };

                if condition != Condition::NoOp {
                    return Ok(condition);
                }
            }
            Statement::Not(box_statement) => {
                // NOTE: Should the condition be used?
                let _condition = self.execute_statement(box_statement)?;
                self.previous_status.toggle();
                self.variables.set("?", self.previous_status);
            }
            Statement::Break => return Ok(Condition::Break),
            Statement::Continue => return Ok(Condition::Continue),
            Statement::Match { expression, cases } => {
                let condition = self.execute_match(expression, cases)?;

                if condition != Condition::NoOp {
                    return Ok(condition);
                }
            }
            _ => {}
        }
        if let Some(signal) = signals::SignalHandler.next() {
            self.handle_signal(signal).map_err(PipelineError::KillFailed)?;
            Err(IonError::from(PipelineError::Interrupted(Pid::this(), signal)))
        } else {
            Ok(Condition::NoOp)
        }
    }

    /// Simply executes all supplied statements.
    pub fn execute_statements(&mut self, statements: &[Statement<'a>]) -> Result {
        self.variables.new_scope(false);

        let condition = statements
            .iter()
            .map(|statement| self.execute_statement(statement))
            .find(|condition| if let Ok(Condition::NoOp) = condition { false } else { true })
            .unwrap_or(Ok(Condition::NoOp));

        self.variables.pop_scope();

        condition
    }

    /// Expand an expression and run a branch based on the value of the
    /// expanded expression
    fn execute_match<T: AsRef<str>>(&mut self, expression: T, cases: &[Case<'a>]) -> Result {
        // Logic for determining if the LHS of a match-case construct (the value we are
        // matching against) matches the RHS of a match-case construct (a value
        // in a case statement). For example, checking to see if the value
        // "foo" matches the pattern "bar" would be invoked like so :
        // ```ignore
        // matches("foo", "bar")
        // ```
        let is_array = is_array(expression.as_ref());
        let value = self.expand_string(expression.as_ref())?;
        for case in cases.iter() {
            if case
                .value
                .as_ref()
                .and_then(|v| self.expand_string(v).ok())
                .filter(|v| v.iter().all(|v| !value.contains(v)))
                .is_none()
            {
                // let pattern_is_array = is_array(&value);
                let previous_bind = case.binding.as_ref().and_then(|bind| {
                    if is_array {
                        let out = if let Some(Value::Array(array)) = self.variables.get(bind) {
                            Some(Value::Array(array.clone()))
                        } else {
                            None
                        };
                        self.variables_mut()
                            .set(bind, value.iter().cloned().map(Value::Str).collect::<Value<_>>());
                        out
                    } else {
                        let out = if let Some(Value::Str(val)) = self.variables.get(bind) {
                            Some(Value::Str(val.clone()))
                        } else {
                            None
                        };
                        self.variables_mut().set(bind, value.join(" "));
                        out
                    }
                });

                if let Some(statement) = case.conditional.as_ref() {
                    self.on_command(statement)?;
                    if self.previous_status.is_failure() {
                        continue;
                    }
                }

                let condition = self.execute_statements(&case.statements);

                if let Some(ref bind) = case.binding {
                    if let Some(value) = previous_bind {
                        match value {
                            Value::HashMap(_) | Value::Array(_) | Value::Str(_) => {
                                self.variables_mut().set(bind, value);
                            }
                            _ => (),
                        }
                    }
                }

                return condition;
            }
        }

        Ok(Condition::NoOp)
    }

    /// Receives a command and attempts to execute the contents.
    pub fn on_command(&mut self, command_string: &str) -> std::result::Result<(), IonError> {
        for stmt in command_string.bytes().batching(|cmd| Terminator::new(cmd).terminate()) {
            // Go through all of the statements and build up the block stack
            // When block is done return statement for execution.
            for statement in StatementSplitter::new(&stmt) {
                let statement = parse_and_validate(statement?, &self.builtins)?;
                if let Some(stm) = Self::insert_statement(&mut self.flow_control, statement)? {
                    self.execute_statement(&stm)?;
                }
            }
        }
        Ok(())
    }
}

/// Expand a pipeline containing aliases. As aliases can split the pipeline by having logical
/// operators in them, the function returns the first half of the pipeline and the rest of the
/// statements, where the last statement has the other half of the pipeline merged.
// TODO: If the aliases are made standard functions, the error type must be changed
fn expand_pipeline<'a>(
    shell: &Shell<'a>,
    pipeline: &Pipeline<Job<'a>>,
) -> std::result::Result<(Pipeline<Job<'a>>, Vec<Statement<'a>>), IonError> {
    let mut item_iter = pipeline.items.iter();
    let mut items: Vec<PipeItem<Job<'a>>> = Vec::with_capacity(item_iter.size_hint().0);
    let mut statements = Vec::new();

    while let Some(item) = item_iter.next() {
        if let Some(Value::Alias(alias)) = shell.variables.get(&item.job.args[0]) {
            statements = StatementSplitter::new(alias.0.as_str())
                .map(|stmt| parse_and_validate(stmt?, &shell.builtins).map_err(Into::into))
                .collect::<std::result::Result<_, IonError>>()?;

            // First item in the alias should be a pipeline item, otherwise it cannot
            // be placed into a pipeline!
            let len = statements.len();
            if let Some(Statement::Pipeline(ref mut pline)) = statements.first_mut() {
                // Connect inputs and outputs of alias to pipeline
                if let Some(first) = pline.items.first_mut() {
                    first.inputs = item.inputs.clone();

                    // Add alias arguments to expanded args if there's any.
                    first.job.args.extend(item.job.args.iter().skip(1).cloned());
                }
                if len == 1 {
                    if let Some(mut last) = pline.items.last_mut() {
                        last.outputs = item.outputs.clone();
                        last.job.redirection = item.job.redirection;
                    }
                }
                items.append(&mut pline.items);
                statements.remove(0);
            }

            // Handle pipeline being broken half by i.e.: '&&' or '||'
            if !statements.is_empty() {
                match statements.last_mut().unwrap() {
                    Statement::And(ref mut boxed_stm)
                    | Statement::Or(ref mut boxed_stm)
                    | Statement::Not(ref mut boxed_stm)
                    | Statement::Time(ref mut boxed_stm) => {
                        if let Statement::Pipeline(ref mut pline) = &mut **boxed_stm {
                            // Set output of alias to be the output of last pipeline.
                            if let Some(last) = pline.items.last_mut() {
                                last.outputs = item.outputs.clone();
                                last.job.redirection = item.job.redirection;
                            }
                            // Append rest of the pipeline to the last pipeline in the
                            // alias.
                            pline.items.extend(item_iter.cloned());
                        } else {
                            // Error in expansion
                            Err(PipelineError::InvalidAlias(
                                item.job.args[0].to_string(),
                                alias.0.to_string(),
                            ))?;
                        }
                    }
                    _ => (),
                }
                break;
            }
        } else {
            items.push(item.clone());
        }
    }
    Ok((Pipeline { items, pipe: pipeline.pipe }, statements))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_match() -> Statement<'static> {
        Statement::Match { expression: types::Str::from(""), cases: Vec::new() }
    }
    fn new_if() -> Statement<'static> {
        Statement::If {
            expression: vec![Statement::Default],
            success:    Vec::new(),
            else_if:    Vec::new(),
            failure:    Vec::new(),
            mode:       IfMode::Success,
        }
    }
    fn new_case() -> Statement<'static> {
        Statement::Case(Case {
            value:       None,
            binding:     None,
            conditional: None,
            statements:  Vec::new(),
        })
    }

    #[test]
    fn if_inside_match() {
        let mut flow_control = Block::default();

        let res = Shell::insert_statement(&mut flow_control, new_match());
        assert_eq!(flow_control.len(), 1);
        assert_eq!(res, Ok(None));

        let res = Shell::insert_statement(&mut flow_control, new_case());
        assert_eq!(flow_control.len(), 2);
        assert_eq!(res, Ok(None));

        // Pops back top case, len stays 2
        let res = Shell::insert_statement(&mut flow_control, new_case());
        assert_eq!(flow_control.len(), 2);
        assert_eq!(res, Ok(None));

        let res = Shell::insert_statement(&mut flow_control, new_if());
        assert_eq!(flow_control.len(), 3);
        assert_eq!(res, Ok(None));

        let res = Shell::insert_statement(&mut flow_control, Statement::End);
        assert_eq!(flow_control.len(), 2);
        assert_eq!(res, Ok(None));

        let res = Shell::insert_statement(&mut flow_control, Statement::End);
        assert_eq!(flow_control.len(), 0);
        if let Ok(Some(Statement::Match { ref cases, .. })) = res {
            assert_eq!(cases.len(), 2);
            assert_eq!(cases.last().unwrap().statements.len(), 1);
        } else {
            panic!();
        }
    }

    #[test]
    fn statement_outside_case() {
        let mut flow_control = Block::default();

        let res = Shell::insert_statement(&mut flow_control, new_match());
        assert_eq!(flow_control.len(), 1);
        assert_eq!(res, Ok(None));

        let res = Shell::insert_statement(&mut flow_control, Statement::Default);
        if res.is_err() {
            flow_control.clear();
            assert_eq!(flow_control.len(), 0);
        } else {
            panic!();
        }
    }

    #[test]
    fn return_toplevel() {
        let mut flow_control = Block::default();
        let oks = vec![
            Statement::Time(Box::new(Statement::Default)),
            Statement::And(Box::new(Statement::Default)),
            Statement::Or(Box::new(Statement::Default)),
            Statement::Not(Box::new(Statement::Default)),
            Statement::Default,
        ];
        for ok in oks {
            let res = Shell::insert_statement(&mut flow_control, ok.clone());
            assert_eq!(Ok(Some(ok)), res);
        }

        let errs = vec![Statement::Else, Statement::End, Statement::Break, Statement::Continue];
        for err in errs {
            assert!(Shell::insert_statement(&mut flow_control, err).is_err());
        }
    }
}
