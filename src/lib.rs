#![doc = include_str!("../README.md")]

mod command_builder;
mod command_spec;
mod config;
mod pipe_builder;
mod pipeline_executor;
mod registration;
mod util;

pub use command_builder::CommandBuilder;
pub use config::Config;
pub use pipe_builder::PipeBuilder;
pub use pipeline_executor::PipelineExecutor;
pub use registration::{builder_module, module, register, ProcessPackage};

#[cfg(feature = "no_index")]
use rhai::Dynamic;
use rhai::EvalAltResult;

#[cfg(not(feature = "no_index"))]
pub(crate) type RhaiArray = rhai::Array;
#[cfg(feature = "no_index")]
pub(crate) type RhaiArray = Vec<Dynamic>;

type RhaiResult<T> = Result<T, Box<EvalAltResult>>;
