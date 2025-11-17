use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub(crate) struct CommandSpec {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) env: BTreeMap<String, String>,
}

impl CommandSpec {
    pub(crate) fn new(program: String, args: Vec<String>) -> Self {
        Self {
            program,
            args,
            env: BTreeMap::new(),
        }
    }
}
