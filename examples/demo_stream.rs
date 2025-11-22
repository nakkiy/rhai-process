use rhai::packages::Package;
use rhai::{Engine, EvalAltResult};
use rhai_process::{Config, ProcessPackage};

fn main() -> Result<(), Box<EvalAltResult>> {
    let mut engine = Engine::new();

    let package = ProcessPackage::new(Config::default());
    package.register_into_engine(&mut engine);

    let contents = engine.eval::<String>(
        r#"
        let result = cmd(["docker", "run", "--rm", "-it", "--name", "demo", "alpine:3.20", "/bin/sh"])
                        .build()
                        .run_stream();

        if result.success {
            result.stdout
        }
        "#,
    )?;
    println!("{}", contents);

    Ok(())
}
