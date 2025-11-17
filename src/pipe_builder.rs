use crate::command_builder::CommandBuilder;
use crate::command_spec::CommandSpec;
use crate::config::Config;
use crate::pipeline_executor::PipelineExecutor;
use crate::util::ensure_same_config;
use crate::RhaiResult;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct PipeBuilder {
    pub(crate) config: Arc<Config>,
    pub(crate) commands: Vec<CommandSpec>,
}

impl PipeBuilder {
    pub(crate) fn from_single(config: Arc<Config>, command: CommandSpec) -> Self {
        Self {
            config,
            commands: vec![command],
        }
    }

    pub(crate) fn push_command(&mut self, spec: CommandSpec) {
        self.commands.push(spec);
    }

    pub(crate) fn into_executor(self) -> PipelineExecutor {
        PipelineExecutor::new(self.config, self.commands)
    }

    pub fn pipe(mut self, next: CommandBuilder) -> RhaiResult<Self> {
        ensure_same_config(&self.config, &next.config)?;
        self.push_command(next.command);
        Ok(self)
    }

    pub fn exec(self) -> PipelineExecutor {
        self.into_executor()
    }
}
