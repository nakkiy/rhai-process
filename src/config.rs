use crate::util::runtime_error;
use crate::RhaiResult;
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct Config {
    pub(crate) command_policy: ListPolicy,
    pub(crate) env_policy: ListPolicy,
    pub(crate) default_timeout_ms: Option<u64>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            command_policy: ListPolicy::Unrestricted,
            env_policy: ListPolicy::Unrestricted,
            default_timeout_ms: None,
        }
    }
}

impl Config {
    pub fn allow_commands<I, S>(mut self, commands: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.command_policy
            .insert_allow(commands.into_iter().map(Into::into));
        self
    }

    pub fn deny_commands<I, S>(mut self, commands: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.command_policy
            .insert_deny(commands.into_iter().map(Into::into));
        self
    }

    pub fn allow_env_vars<I, S>(mut self, keys: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.env_policy
            .insert_allow(keys.into_iter().map(Into::into));
        self
    }

    pub fn deny_env_vars<I, S>(mut self, keys: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.env_policy
            .insert_deny(keys.into_iter().map(Into::into));
        self
    }

    pub fn default_timeout_ms(mut self, timeout: u64) -> Self {
        if timeout == 0 {
            panic!("default_timeout_ms must be greater than zero");
        }
        self.default_timeout_ms = Some(timeout);
        self
    }

    pub(crate) fn ensure_command_allowed(&self, name: &str) -> RhaiResult<()> {
        if self.command_policy.is_allowed(name) {
            Ok(())
        } else {
            Err(runtime_error(format!("command '{name}' is not permitted")))
        }
    }

    pub(crate) fn ensure_env_allowed(&self, key: &str) -> RhaiResult<()> {
        if self.env_policy.is_allowed(key) {
            Ok(())
        } else {
            Err(runtime_error(format!(
                "environment variable '{key}' is not permitted"
            )))
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ListPolicy {
    Unrestricted,
    Allow(HashSet<String>),
    Deny(HashSet<String>),
}

impl ListPolicy {
    fn insert_allow<I>(&mut self, values: I)
    where
        I: IntoIterator<Item = String>,
    {
        match self {
            ListPolicy::Unrestricted => {
                let mut set = HashSet::new();
                set.extend(values);
                *self = ListPolicy::Allow(set);
            }
            ListPolicy::Allow(existing) => existing.extend(values),
            ListPolicy::Deny(_) => {
                panic!("deny list already specified; allow list cannot be combined")
            }
        }
    }

    fn insert_deny<I>(&mut self, values: I)
    where
        I: IntoIterator<Item = String>,
    {
        match self {
            ListPolicy::Unrestricted => {
                let mut set = HashSet::new();
                set.extend(values);
                *self = ListPolicy::Deny(set);
            }
            ListPolicy::Deny(existing) => existing.extend(values),
            ListPolicy::Allow(_) => {
                panic!("allow list already specified; deny list cannot be combined")
            }
        }
    }

    fn is_allowed(&self, value: &str) -> bool {
        match self {
            ListPolicy::Unrestricted => true,
            ListPolicy::Allow(list) => list.contains(value),
            ListPolicy::Deny(list) => !list.contains(value),
        }
    }
}
