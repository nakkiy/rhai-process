use crate::command_builder::CommandBuilder;
use crate::config::Config;
use crate::pipe_builder::PipeBuilder;
use crate::pipeline_executor::PipelineExecutor;
use crate::RhaiArray;
use rhai::packages::Package;
use rhai::plugin::*;
use rhai::{Engine, FnPtr, ImmutableString, Map as RhaiMap, Module, NativeCallContext, Shared};
use std::sync::Arc;

pub fn module(config: Config) -> Module {
    let shared = Arc::new(config);
    let mut module = Module::new();
    attach_custom_types(&mut module);

    {
        let config = Arc::clone(&shared);
        module.set_native_fn("cmd", move |args: RhaiArray| {
            CommandBuilder::new(Arc::clone(&config), args)
        });
    }

    module
}

pub fn register(engine: &mut Engine, config: Config) {
    ProcessPackage::new(config).register_into_engine(engine);
}

pub fn builder_module() -> Module {
    let mut module = exported_module!(builder_api_module);
    attach_custom_types(&mut module);
    module
}

#[derive(Clone)]
pub struct ProcessPackage {
    builder_module: Shared<Module>,
    process_module: Shared<Module>,
}

impl ProcessPackage {
    pub fn new(config: Config) -> Self {
        Self {
            builder_module: builder_module().into(),
            process_module: module(config).into(),
        }
    }
}

impl Package for ProcessPackage {
    fn init(_: &mut Module) {}

    fn as_shared_module(&self) -> Shared<Module> {
        self.builder_module.clone()
    }

    fn register_into_engine(&self, engine: &mut Engine) -> &Self {
        engine.register_global_module(self.builder_module.clone());
        engine.register_global_module(self.process_module.clone());
        self
    }
}

fn attach_custom_types(module: &mut Module) {
    module.set_custom_type::<CommandBuilder>("CommandBuilder");
    module.set_custom_type::<PipeBuilder>("PipeBuilder");
    module.set_custom_type::<PipelineExecutor>("PipelineExecutor");
}

#[export_module]
pub mod builder_api_module {
    use super::*;

    #[rhai_fn(name = "env", return_raw)]
    pub fn builder_env(builder: CommandBuilder, map: RhaiMap) -> crate::RhaiResult<CommandBuilder> {
        builder.with_env_map(map)
    }

    #[rhai_fn(name = "env_var", return_raw)]
    pub fn builder_env_var(
        builder: CommandBuilder,
        key: ImmutableString,
        value: ImmutableString,
    ) -> crate::RhaiResult<CommandBuilder> {
        builder.with_env_var(key.into(), value.into())
    }

    #[rhai_fn(name = "pipe", return_raw)]
    pub fn builder_pipe(
        builder: CommandBuilder,
        next: CommandBuilder,
    ) -> crate::RhaiResult<PipeBuilder> {
        builder.pipe(next)
    }

    #[rhai_fn(name = "build")]
    pub fn builder_build(builder: CommandBuilder) -> PipelineExecutor {
        builder.build()
    }

    #[rhai_fn(name = "pipe", return_raw)]
    pub fn pipeline_pipe(
        pipeline: PipeBuilder,
        next: CommandBuilder,
    ) -> crate::RhaiResult<PipeBuilder> {
        pipeline.pipe(next)
    }

    #[rhai_fn(name = "build")]
    pub fn pipeline_build(pipeline: PipeBuilder) -> PipelineExecutor {
        pipeline.build()
    }

    #[rhai_fn(name = "cwd", return_raw)]
    pub fn executor_cwd(
        executor: PipelineExecutor,
        path: ImmutableString,
    ) -> crate::RhaiResult<PipelineExecutor> {
        executor.cwd(path.into())
    }

    #[rhai_fn(name = "timeout", return_raw)]
    pub fn executor_timeout(
        executor: PipelineExecutor,
        timeout: rhai::INT,
    ) -> crate::RhaiResult<PipelineExecutor> {
        executor.timeout(timeout)
    }

    #[rhai_fn(name = "allow_exit_codes", return_raw)]
    pub fn executor_exit_codes(
        executor: PipelineExecutor,
        codes: RhaiArray,
    ) -> crate::RhaiResult<PipelineExecutor> {
        executor.allow_exit_codes(codes)
    }

    #[rhai_fn(name = "run", return_raw)]
    pub fn executor_run(executor: PipelineExecutor) -> crate::RhaiResult<RhaiMap> {
        executor.run()
    }

    #[rhai_fn(name = "run_stream", return_raw)]
    pub fn executor_run_stream_default(
        context: NativeCallContext,
        executor: PipelineExecutor,
    ) -> crate::RhaiResult<RhaiMap> {
        executor.run_stream(&context, None, None)
    }

    #[rhai_fn(name = "run_stream", return_raw)]
    pub fn executor_run_stream_stdout(
        context: NativeCallContext,
        executor: PipelineExecutor,
        stdout_cb: FnPtr,
    ) -> crate::RhaiResult<RhaiMap> {
        executor.run_stream(&context, Some(stdout_cb), None)
    }

    #[rhai_fn(name = "run_stream", return_raw)]
    pub fn executor_run_stream_both(
        context: NativeCallContext,
        executor: PipelineExecutor,
        stdout_cb: FnPtr,
        stderr_cb: FnPtr,
    ) -> crate::RhaiResult<RhaiMap> {
        executor.run_stream(&context, Some(stdout_cb), Some(stderr_cb))
    }
}
