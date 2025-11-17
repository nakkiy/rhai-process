use crate::command_spec::CommandSpec;
use crate::config::Config;
use crate::util::{map_io_err, normalize_exit_codes, runtime_error};
use crate::{RhaiArray, RhaiResult};
use duct::{self, Expression};
use rhai::{Dynamic, Map as RhaiMap, INT};
use std::collections::HashSet;
use std::io;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct PipelineExecutor {
    pub(crate) config: Arc<Config>,
    pub(crate) commands: Vec<CommandSpec>,
    pub(crate) timeout_override_ms: Option<u64>,
    pub(crate) allowed_exit_codes: Option<HashSet<i64>>,
}

impl PipelineExecutor {
    pub(crate) fn new(config: Arc<Config>, commands: Vec<CommandSpec>) -> Self {
        Self {
            config,
            commands,
            timeout_override_ms: None,
            allowed_exit_codes: None,
        }
    }

    pub fn timeout(mut self, timeout: INT) -> RhaiResult<Self> {
        if timeout <= 0 {
            return Err(runtime_error("timeout must be a positive integer"));
        }
        self.timeout_override_ms = Some(timeout as u64);
        Ok(self)
    }

    pub fn allow_exit_codes(mut self, codes: RhaiArray) -> RhaiResult<Self> {
        let mut set = HashSet::new();
        for code in codes {
            let value = code
                .clone()
                .try_cast::<INT>()
                .ok_or_else(|| runtime_error("allow_exit_codes expects integers"))?;
            set.insert(value as i64);
        }
        self.allowed_exit_codes = normalize_exit_codes(set);
        Ok(self)
    }

    pub fn capture(self) -> RhaiResult<RhaiMap> {
        let timeout = self.timeout_override_ms.or(self.config.default_timeout_ms);
        let result = run_pipeline(&self.commands, timeout, self.allowed_exit_codes.clone())?;
        Ok(result.into_map())
    }
}

#[derive(Debug)]
struct ProcessResult {
    success: bool,
    status: i64,
    stdout: String,
    stderr: String,
    duration_ms: u64,
}

impl ProcessResult {
    fn into_map(self) -> RhaiMap {
        let mut map = RhaiMap::new();
        map.insert("success".into(), Dynamic::from_bool(self.success));
        map.insert("status".into(), Dynamic::from_int(self.status as INT));
        map.insert("stdout".into(), Dynamic::from(self.stdout));
        map.insert("stderr".into(), Dynamic::from(self.stderr));
        let duration_int: INT = self.duration_ms.try_into().unwrap_or(i64::MAX);
        map.insert("duration_ms".into(), Dynamic::from_int(duration_int));
        map
    }
}

fn run_pipeline(
    commands: &[CommandSpec],
    timeout_ms: Option<u64>,
    allowed_exit_codes: Option<HashSet<i64>>,
) -> RhaiResult<ProcessResult> {
    if commands.is_empty() {
        return Err(runtime_error("no command specified"));
    }
    let mut expression = build_expression(commands)?;
    expression = expression.stdout_capture().stderr_capture().unchecked();
    let start = Instant::now();
    let output = match timeout_ms {
        Some(ms) => run_with_timeout(expression, Duration::from_millis(ms)).map_err(map_io_err)?,
        None => expression.run().map_err(map_io_err)?,
    };
    let duration = start.elapsed();
    let exit_code = output.status.code().map(|c| c as i64).unwrap_or(-1);
    let mut success = output.status.success();
    if !success {
        if let Some(allowed) = allowed_exit_codes.as_ref() {
            if allowed.contains(&exit_code) {
                success = true;
            }
        }
    }

    Ok(ProcessResult {
        success,
        status: exit_code,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        duration_ms: duration.as_millis().try_into().unwrap_or(u64::MAX),
    })
}

fn build_expression(commands: &[CommandSpec]) -> RhaiResult<Expression> {
    let mut iter = commands.iter();
    let first = iter
        .next()
        .ok_or_else(|| runtime_error("no command specified"))?;
    let mut expression = expression_from_spec(first);
    for command in iter {
        let next_expr = expression_from_spec(command);
        expression = expression.pipe(next_expr);
    }
    Ok(expression)
}

fn expression_from_spec(spec: &CommandSpec) -> Expression {
    let mut expr = duct::cmd(spec.program.clone(), spec.args.clone());
    if let Some(cwd) = &spec.cwd {
        expr = expr.dir(cwd.clone());
    }
    for (key, value) in &spec.env {
        expr = expr.env(key, value);
    }
    expr
}

fn run_with_timeout(expr: Expression, limit: Duration) -> io::Result<std::process::Output> {
    let handle = expr.start()?;
    let start = Instant::now();
    loop {
        if let Some(output) = handle.try_wait()? {
            return Ok(std::process::Output {
                status: output.status.clone(),
                stdout: output.stdout.clone(),
                stderr: output.stderr.clone(),
            });
        }

        if start.elapsed() >= limit {
            handle.kill()?;
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "process execution timed out",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}
