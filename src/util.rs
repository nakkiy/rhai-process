use crate::config::Config;
use crate::RhaiResult;
use rhai::{Dynamic, EvalAltResult, ImmutableString, Position};
use std::collections::HashSet;
use std::io;
use std::sync::Arc;

pub(crate) fn runtime_error(msg: impl Into<String>) -> Box<EvalAltResult> {
    EvalAltResult::ErrorRuntime(Dynamic::from(msg.into()), Position::NONE).into()
}

pub(crate) fn map_io_err(err: io::Error) -> Box<EvalAltResult> {
    runtime_error(format!("process I/O error: {err}"))
}

pub(crate) fn dynamic_to_string(value: Dynamic, label: &str) -> RhaiResult<String> {
    value
        .try_cast::<ImmutableString>()
        .map(|s| s.into())
        .ok_or_else(|| runtime_error(format!("{label} must be a string")))
}

pub(crate) fn ensure_same_config(a: &Arc<Config>, b: &Arc<Config>) -> RhaiResult<()> {
    if Arc::ptr_eq(a, b) {
        Ok(())
    } else {
        Err(runtime_error(
            "command builders come from different modules",
        ))
    }
}

pub(crate) fn normalize_exit_codes(set: HashSet<i64>) -> Option<HashSet<i64>> {
    if set.is_empty() {
        None
    } else {
        Some(set)
    }
}
