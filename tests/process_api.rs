use rhai::{Engine, EvalAltResult};
use rhai_process::{register, Config};
use tempfile::tempdir;

fn engine_with(config: Config) -> Engine {
    let mut engine = Engine::new();
    register(&mut engine, config);
    engine
}

fn eval_bool(engine: &Engine, script: &str) -> Result<bool, Box<EvalAltResult>> {
    engine.eval(script)
}

#[test]
fn capture_returns_stdout_and_status() -> Result<(), Box<EvalAltResult>> {
    let engine = engine_with(Config::default());
    let script = r#"
        let result = process::cmd(["python3", "-c", "print('hello')"])
            .exec()
            .capture();
        result.success && result.status == 0 && result.stdout.contains("hello")
    "#;
    assert!(
        eval_bool(&engine, script)?,
        "capture output should include python text"
    );
    Ok(())
}

#[test]
fn pipeline_passes_stdout() -> Result<(), Box<EvalAltResult>> {
    let engine = engine_with(Config::default());
    let script = r#"
        let result = process::cmd(["python3", "-c", "print('foo')"]).pipe(
            process::cmd(["python3", "-c", "import sys; data=sys.stdin.read(); sys.stdout.write(data.upper())"])
        ).exec().capture();
        result.stdout.contains("FOO")
    "#;
    assert!(eval_bool(&engine, script)?, "pipe should transform stdout");
    Ok(())
}

#[test]
fn global_cmd_alias_available() -> Result<(), Box<EvalAltResult>> {
    let engine = engine_with(Config::default());
    let script = r#"
        let result = cmd(["python3", "-c", "print('hi')"]).exec().capture();
        result.success && result.stdout.contains("hi")
    "#;
    assert!(eval_bool(&engine, script)?);
    Ok(())
}

#[test]
fn allow_commands_whitelist() {
    let engine = engine_with(Config::default().allow_commands(["python3"]));
    let script = r#"
        process::cmd(["ls"]).exec().capture();
        true
    "#;
    let err = engine
        .eval::<bool>(script)
        .expect_err("ls should be blocked");
    assert!(err.to_string().contains("not permitted"));
}

#[test]
fn deny_commands_blacklist() {
    let engine = engine_with(Config::default().deny_commands(["ls"]));
    let script = r#"
        process::cmd(["ls"]).exec().capture();
        true
    "#;
    let err = engine
        .eval::<bool>(script)
        .expect_err("ls should be denied");
    assert!(err.to_string().contains("not permitted"));
}

#[test]
fn env_injection_and_whitelist() -> Result<(), Box<EvalAltResult>> {
    let engine = engine_with(Config::default().allow_env_vars(["RHAI_PROCESS_TEST"]));
    let script = r#"
        let result = process::cmd(["env"]).env(#{ "RHAI_PROCESS_TEST": "ok" }).exec().capture();
        result.stdout.contains("RHAI_PROCESS_TEST=ok")
    "#;
    assert!(eval_bool(&engine, script)?);

    let forbidden = r#"
        process::cmd(["env"]).env(#{ "OTHER": "nope" }).exec().capture();
        true
    "#;
    let err = engine
        .eval::<bool>(forbidden)
        .expect_err("OTHER should be blocked");
    assert!(err.to_string().contains("not permitted"));
    Ok(())
}

#[test]
fn deny_env_vars_blocks_key() {
    let engine = engine_with(Config::default().deny_env_vars(["BLOCKED"]));
    let script = r#"
        process::cmd(["env"]).env(#{ "BLOCKED": "1" }).exec().capture();
        true
    "#;
    let err = engine
        .eval::<bool>(script)
        .expect_err("BLOCKED should be denied");
    assert!(err.to_string().contains("not permitted"));
}

#[test]
fn env_var_sets_single_entry() -> Result<(), Box<EvalAltResult>> {
    let engine = engine_with(Config::default().allow_env_vars(["SINGLE_VAR"]));
    let script = r#"
        let result = process::cmd(["env"]).env_var("SINGLE_VAR", "value").exec().capture();
        result.stdout.contains("SINGLE_VAR=value")
    "#;
    assert!(eval_bool(&engine, script)?);
    Ok(())
}

#[test]
fn allow_exit_codes_mark_success() -> Result<(), Box<EvalAltResult>> {
    let engine = engine_with(Config::default());
    let script = r#"
        let result = process::cmd(["false"]).exec().allow_exit_codes([1]).capture();
        result.success
    "#;
    assert!(
        eval_bool(&engine, script)?,
        "exit code 1 should be tolerated"
    );
    Ok(())
}

#[test]
fn default_timeout_triggers_error() {
    let engine = engine_with(Config::default().default_timeout_ms(100));
    let script = r#"
        process::cmd(["python3", "-c", "import time; time.sleep(1)"]).exec().capture();
        true
    "#;
    let err = engine.eval::<bool>(script).expect_err("should time out");
    assert!(err.to_string().contains("timed out") || err.to_string().contains("I/O error"));
}

#[test]
fn capture_reports_duration() -> Result<(), Box<EvalAltResult>> {
    let engine = engine_with(Config::default());
    let script = r#"
        let result = process::cmd(["python3", "-c", "print('ok')"]).exec().capture();
        result.duration_ms >= 0
    "#;
    assert!(eval_bool(&engine, script)?);
    Ok(())
}

#[test]
fn cwd_switches_directory() -> Result<(), Box<EvalAltResult>> {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("hello.txt");
    std::fs::write(&file_path, "hi").expect("write temp file");
    let dir_str = dir.path().to_str().unwrap();
    let script = format!(
        r#"
        let result = process::cmd(["ls"]).cwd("{dir}").exec().capture();
        result.stdout.contains("hello.txt")
        "#,
        dir = dir_str
    );
    let engine = engine_with(Config::default());
    assert!(eval_bool(&engine, &script)?);
    Ok(())
}

#[test]
fn cwd_invalid_directory_errors() {
    let engine = engine_with(Config::default());
    let script = r#"
        process::cmd(["ls"]).cwd("/definitely/not/a/dir").exec().capture();
        true
    "#;
    let err = engine
        .eval::<bool>(script)
        .expect_err("invalid cwd should fail");
    assert!(err.to_string().contains("I/O error") || err.to_string().contains("timed out"));
}

#[test]
fn per_command_timeout_applies() {
    let engine = engine_with(Config::default());
    let script = r#"
        process::cmd(["python3", "-c", "import time; time.sleep(1)"])
            .exec()
            .timeout(100)
            .capture();
        true
    "#;
    let err = engine
        .eval::<bool>(script)
        .expect_err("per-command timeout should trigger");
    assert!(err.to_string().contains("timed out") || err.to_string().contains("I/O error"));
}

#[test]
#[should_panic(expected = "default_timeout_ms must be greater than zero")]
fn default_timeout_zero_rejected() {
    let _ = Config::default().default_timeout_ms(0);
}
