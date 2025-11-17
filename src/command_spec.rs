use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub(crate) struct CommandSpec {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) env: BTreeMap<String, String>,
}

impl CommandSpec {
    pub(crate) fn new(program: String, args: Vec<String>) -> Self {
        Self {
            program,
            args,
            cwd: None,
            env: BTreeMap::new(),
        }
    }
}
