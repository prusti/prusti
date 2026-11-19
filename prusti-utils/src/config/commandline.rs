use ::config::{ConfigError, Source, Value};
// Cannot use rustc_hash::FxHashMap here, due to type requirement in config::Source
use std::{collections::HashMap, env};

/// The configuration flags given as command-line arguments. `get_remaining_args` returns the
/// other arguments.
#[derive(Clone, Debug)]
pub struct CommandLine {
    /// Prefix that limits args to those that begin with it.
    ///
    /// Example: The arg -Zdebug=true would become debug=true with a prefix of -Z
    prefix: String,
}

impl CommandLine {
    pub fn with_prefix(s: &str) -> Self {
        CommandLine {
            prefix: s.to_owned(),
        }
    }

    /// Return String iterator of arguments that are not flags.
    pub fn get_remaining_args(self) -> impl Iterator<Item = String> {
        env::args().filter(move |arg| self.key_value(arg).is_none())
    }

    /// The key and value of the flag `arg`: an argument is a flag if it begins with the prefix
    /// and the first occurrence of '=' after it separates two non-empty strings.
    fn key_value<'a>(&self, arg: &'a str) -> Option<(&'a str, &'a str)> {
        let (key, value) = arg.strip_prefix(&self.prefix)?.split_once('=')?;
        (!key.is_empty() && !value.is_empty()).then_some((key, value))
    }
}

impl Source for CommandLine {
    fn clone_into_box(&self) -> Box<dyn Source + Send + Sync> {
        Box::new((*self).clone())
    }

    fn collect(&self) -> Result<HashMap<String, Value>, ConfigError> {
        let uri = String::from("command-line");
        Ok(env::args()
            .filter_map(|arg| {
                let (key, value) = self.key_value(&arg)?;
                Some((key.to_lowercase(), Value::new(Some(&uri), value)))
            })
            .collect())
    }
}
