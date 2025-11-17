use rhai::packages::Package;
use rhai::{Engine, EvalAltResult};
use rhai_process::{Config, ProcessPackage};

fn main() -> Result<(), Box<EvalAltResult>> {
    let mut engine = Engine::new();

    let package = ProcessPackage::new(Config::default());
    package.register_into_engine(&mut engine);

    let contents = engine.eval::<String>(
        r#"
        let result = cmd(["sleep", "2"])
                        .exec()
                        .timeout(1_000)
                        .capture();

        if result.success {
            result.stdout
        }
        "#,
    )?;
    println!("{}", contents);

    Ok(())
}
