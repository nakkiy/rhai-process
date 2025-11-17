use crate::command_spec::CommandSpec;
use crate::config::Config;
use crate::pipe_builder::PipeBuilder;
use crate::pipeline_executor::PipelineExecutor;
use crate::util::{dynamic_to_string, runtime_error};
use crate::{RhaiArray, RhaiResult};
use rhai::Map as RhaiMap;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct CommandBuilder {
    pub(crate) config: Arc<Config>,
    pub(crate) command: CommandSpec,
}

impl CommandBuilder {
    pub(crate) fn new(config: Arc<Config>, args: RhaiArray) -> RhaiResult<Self> {
        if args.is_empty() {
            return Err(runtime_error("process::cmd requires at least one argument"));
        }

        let mut items = args.into_iter();
        let program = dynamic_to_string(
            items.next().expect("non-empty array ensured"),
            "command name",
        )?;
        config.ensure_command_allowed(&program)?;
        let mut arg_list = Vec::new();
        for arg in items {
            arg_list.push(dynamic_to_string(arg, "command argument")?);
        }

        Ok(Self {
            config,
            command: CommandSpec::new(program, arg_list),
        })
    }

    pub(crate) fn with_env_map(mut self, map: RhaiMap) -> RhaiResult<Self> {
        for (key, value) in map.into_iter() {
            let string_key: String = key.into();
            let string_value = dynamic_to_string(value, "environment value")?;
            self.config.ensure_env_allowed(&string_key)?;
            self.command.env.insert(string_key, string_value);
        }
        Ok(self)
    }

    pub(crate) fn with_env_var(mut self, key: String, value: String) -> RhaiResult<Self> {
        self.config.ensure_env_allowed(&key)?;
        self.command.env.insert(key, value);
        Ok(self)
    }

    pub(crate) fn pipe(self, next: CommandBuilder) -> RhaiResult<PipeBuilder> {
        crate::util::ensure_same_config(&self.config, &next.config)?;
        let mut builder = PipeBuilder::from_single(Arc::clone(&self.config), self.command);
        builder.push_command(next.command);
        Ok(builder)
    }

    pub(crate) fn build(self) -> PipelineExecutor {
        PipeBuilder::from_single(self.config, self.command).into_executor()
    }
}
