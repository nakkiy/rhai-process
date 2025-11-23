use crate::command_spec::CommandSpec;
use crate::config::Config;
use crate::util::{map_io_err, normalize_exit_codes, runtime_error};
use crate::{RhaiArray, RhaiResult};
use duct::{self, Expression};
use os_pipe::PipeReader;
use rhai::{Dynamic, FnPtr, ImmutableString, Map as RhaiMap, NativeCallContext, INT};
use std::collections::HashSet;
use std::io::{self, ErrorKind, Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct PipelineExecutor {
    pub(crate) config: Arc<Config>,
    pub(crate) commands: Vec<CommandSpec>,
    pub(crate) timeout_override_ms: Option<u64>,
    pub(crate) allowed_exit_codes: Option<HashSet<i64>>,
    pub(crate) cwd: Option<PathBuf>,
}

impl PipelineExecutor {
    pub(crate) fn new(config: Arc<Config>, commands: Vec<CommandSpec>) -> Self {
        Self {
            config,
            commands,
            timeout_override_ms: None,
            allowed_exit_codes: None,
            cwd: None,
        }
    }

    pub fn cwd(mut self, path: String) -> RhaiResult<Self> {
        if path.is_empty() {
            self.cwd = None;
        } else {
            self.cwd = Some(PathBuf::from(path));
        }
        Ok(self)
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
            set.insert(value);
        }
        self.allowed_exit_codes = normalize_exit_codes(set);
        Ok(self)
    }

    pub fn run(self) -> RhaiResult<RhaiMap> {
        let timeout = self.timeout_override_ms.or(self.config.default_timeout_ms);
        let result = run_pipeline(
            &self.commands,
            timeout,
            self.allowed_exit_codes.clone(),
            self.cwd,
        )?;
        Ok(result.into_map())
    }

    pub fn run_stream(
        self,
        context: &NativeCallContext,
        stdout_cb: Option<FnPtr>,
        stderr_cb: Option<FnPtr>,
    ) -> RhaiResult<RhaiMap> {
        let timeout = self.timeout_override_ms.or(self.config.default_timeout_ms);
        let result = run_pipeline_stream(
            &self.commands,
            timeout,
            self.allowed_exit_codes.clone(),
            self.cwd,
            context,
            stdout_cb,
            stderr_cb,
        )?;
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
    cwd: Option<PathBuf>,
) -> RhaiResult<ProcessResult> {
    if commands.is_empty() {
        return Err(runtime_error("no command specified"));
    }
    let mut expression = build_expression(commands, cwd.as_ref())?;
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

fn run_pipeline_stream(
    commands: &[CommandSpec],
    timeout_ms: Option<u64>,
    allowed_exit_codes: Option<HashSet<i64>>,
    cwd: Option<PathBuf>,
    context: &NativeCallContext,
    stdout_cb: Option<FnPtr>,
    stderr_cb: Option<FnPtr>,
) -> RhaiResult<ProcessResult> {
    if commands.is_empty() {
        return Err(runtime_error("no command specified"));
    }

    let mut expression = build_expression(commands, cwd.as_ref())?;
    let (stdout_reader, stdout_writer) = os_pipe::pipe().map_err(map_io_err)?;
    let (stderr_reader, stderr_writer) = os_pipe::pipe().map_err(map_io_err)?;
    expression = expression
        .stdout_file(stdout_writer)
        .stderr_file(stderr_writer)
        .unchecked();

    let handle = expression.start().map_err(map_io_err)?;
    drop(expression);
    let start = Instant::now();
    let (tx, rx) = mpsc::channel();
    spawn_stream_reader(stdout_reader, tx.clone(), StreamKind::Stdout);
    spawn_stream_reader(stderr_reader, tx, StreamKind::Stderr);

    let mut stdout_open = true;
    let mut stderr_open = true;
    let mut process_finished = false;

    while stdout_open || stderr_open {
        if let Some(limit) = timeout_ms {
            if start.elapsed() >= Duration::from_millis(limit) {
                handle.kill().ok();
                return Err(map_io_err(io::Error::new(
                    ErrorKind::TimedOut,
                    "process execution timed out",
                )));
            }
        }

        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(StreamMessage::Data(kind, chunk)) => {
                dispatch_stream_chunk(
                    kind,
                    &chunk,
                    context,
                    stdout_cb.as_ref(),
                    stderr_cb.as_ref(),
                )?;
            }
            Ok(StreamMessage::Eof(kind)) => match kind {
                StreamKind::Stdout => stdout_open = false,
                StreamKind::Stderr => stderr_open = false,
            },
            Ok(StreamMessage::Error(err)) => {
                handle.kill().ok();
                return Err(map_io_err(err));
            }
            Err(RecvTimeoutError::Timeout) => {
                if !process_finished && handle.try_wait().map_err(map_io_err)?.is_some() {
                    process_finished = true;
                }
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    let duration = start.elapsed();
    let output = handle.wait().map_err(map_io_err)?;
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
        stdout: String::new(),
        stderr: String::new(),
        duration_ms: duration.as_millis().try_into().unwrap_or(u64::MAX),
    })
}

fn build_expression(commands: &[CommandSpec], cwd: Option<&PathBuf>) -> RhaiResult<Expression> {
    let mut iter = commands.iter();
    let first = iter
        .next()
        .ok_or_else(|| runtime_error("no command specified"))?;
    let mut expression = expression_from_spec(first, cwd);
    for command in iter {
        let next_expr = expression_from_spec(command, cwd);
        expression = expression.pipe(next_expr);
    }
    Ok(expression)
}

fn expression_from_spec(spec: &CommandSpec, cwd: Option<&PathBuf>) -> Expression {
    let mut expr = duct::cmd(spec.program.clone(), spec.args.clone());
    if let Some(dir) = cwd {
        expr = expr.dir(dir.clone());
    }
    for (key, value) in &spec.env {
        expr = expr.env(key, value);
    }
    expr
}

fn run_with_timeout(expr: Expression, limit: Duration) -> io::Result<std::process::Output> {
    let handle = Arc::new(expr.start()?);
    drop(expr);

    let wait_handle = Arc::clone(&handle);
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = wait_handle
            .wait()
            .map(|output| std::process::Output {
                status: output.status,
                stdout: output.stdout.clone(),
                stderr: output.stderr.clone(),
            });
        let _ = tx.send(result);
    });

    match rx.recv_timeout(limit) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => {
            handle.kill()?;
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "process execution timed out",
            ))
        }
        Err(RecvTimeoutError::Disconnected) => Err(io::Error::new(
            io::ErrorKind::Other,
            "process execution failed",
        )),
    }
}

#[derive(Copy, Clone)]
enum StreamKind {
    Stdout,
    Stderr,
}

enum StreamMessage {
    Data(StreamKind, Vec<u8>),
    Eof(StreamKind),
    Error(io::Error),
}

fn spawn_stream_reader(reader: PipeReader, sender: Sender<StreamMessage>, kind: StreamKind) {
    thread::spawn(move || {
        let mut reader = reader;
        let mut buffer = [0u8; 8 * 1024];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => {
                    let _ = sender.send(StreamMessage::Eof(kind));
                    break;
                }
                Ok(n) => {
                    if sender
                        .send(StreamMessage::Data(kind, buffer[..n].to_vec()))
                        .is_err()
                    {
                        break;
                    }
                }
                Err(ref err) if err.kind() == ErrorKind::Interrupted => continue,
                Err(err) => {
                    let _ = sender.send(StreamMessage::Error(err));
                    break;
                }
            }
        }
    });
}

fn dispatch_stream_chunk(
    kind: StreamKind,
    chunk: &[u8],
    context: &NativeCallContext,
    stdout_cb: Option<&FnPtr>,
    stderr_cb: Option<&FnPtr>,
) -> RhaiResult<()> {
    let text = String::from_utf8_lossy(chunk).to_string();
    let value: ImmutableString = text.clone().into();

    let target = match kind {
        StreamKind::Stdout => stdout_cb,
        StreamKind::Stderr => stderr_cb,
    };

    if let Some(callback) = target {
        let _ = callback.call_within_context::<Dynamic>(context, (value,))?;
    } else {
        match kind {
            StreamKind::Stdout => {
                print!("{}", text);
                let _ = io::stdout().flush();
            }
            StreamKind::Stderr => {
                eprint!("{}", text);
                let _ = io::stderr().flush();
            }
        }
    }

    Ok(())
}
