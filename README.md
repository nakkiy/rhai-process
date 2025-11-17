# rhai-process

## Overview
`rhai-process` is a Rust crate that lets Rhai scripts execute external processes in an OS-agnostic and safe way. It focuses on structured array-based commands so CLI/DSL apps can expose a consistent execution interface.

## Rhai Script
```rhai
let result = cmd(["ls"])
                .pipe(cmd(["grep", "Cargo.toml"]))
                .exec()
                .capture();

if result.success {
    print(result.stdout);
}
```

## Rust Source
```rust
use rhai::packages::Package;
use rhai::{Engine, EvalAltResult};
use rhai_process::{Config, ProcessPackage};

fn main() -> Result<(), Box<EvalAltResult>> {
    let mut engine = Engine::new();

    let package = ProcessPackage::new(Config::default());
    package.register_into_engine(&mut engine);

    let contents = engine.eval::<String>(r#"
        let result = cmd(["ls"])
                        .pipe(cmd(["grep", "Cargo.toml"]))
                        .exec()
                        .capture();

        if result.success {
            result.stdout
        }
    "#)?;
    println!("{}", contents);
    Ok(())
}
```

## Config
Host applications use `Config` to control what Rhai scripts may execute.

| Option | Description |
| ------ | ----------- |
| `allow_commands([...])` / `deny_commands([...])` | Whitelist or blacklist executable names (mutually exclusive). When unspecified, all commands are allowed. |
| `allow_env_vars([...])` / `deny_env_vars([...])` | Restrict which environment-variable keys scripts may override (mutually exclusive). Unset means all keys are allowed. |
| `default_timeout_ms(ms)` | Default timeout in milliseconds. Zero or negative values are rejected. Call `Executor::timeout(ms)` to override per pipeline. |
| `allowed_workdirs([...])` | Planned: restrict working directories to the listed paths. Unset means no directory restriction. |

> Every `CommandBuilder` consults this policy before launching. Violations raise an immediate Rhai error and the external process is never started.

## CommandBuilder
```rhai
  let build = cmd(["cargo", "build"])
                 .cwd(repo_dir)
                 .env(#{ "RUSTFLAGS": "-Dwarnings" });
```
| Method | Description |
| ------ | ----------- |
| `cmd([cmd, opt, ...])` | Create a builder by passing the program name and arguments as an array. |
| `cwd(path)` | Set or clear the working directory for this command. |
| `env(map)` / `env_var(key, value)` | Inject environment variables (collectively or individually). Keys must be allowed by `Config`. |
| `pipe(other_builder)` | Append another `CommandBuilder` via a pipe and return a `PipeBuilder`. |
| `exec()` | Turn this single command into an `Executor`, which exposes timeout/exit-code controls and `capture()`. |

## PipeBuilder
| Method | Description |
| ------ | ----------- |
| `pipe(other_builder)` | Attach another command to the current pipeline. |
| `exec()` | Convert the pipeline into an `Executor`. |

## Executor
| Method | Description |
| ------ | ----------- |
| `timeout(ms)` | Override the pipeline-wide timeout in milliseconds (`Config::default_timeout_ms` is used otherwise). |
| `allow_exit_codes(array)` | Treat the listed exit codes as successes. |
| `capture()` | Execute the pipeline and return `#{ success, status, stdout, stderr, duration_ms }`. |

## Handling results
- `capture()` is the sole terminal API. It returns `#{ success, status, stdout, stderr, duration_ms }`; check `success` (or inspect `stderr`) and raise your own error if needed. I/O errors or timeouts still surface as `EvalAltResult`.

## License
Dual-licensed under MIT or Apache-2.0.
