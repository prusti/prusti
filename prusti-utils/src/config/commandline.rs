use ::config::{ConfigError, Source, Value};
use itertools::Itertools;
// Cannot use rustc_hash::FxHashMap here, due to type requirement in config::Source
use std::{collections::HashMap, env};

#[derive(Clone, Debug, Default)]
pub struct CommandLine {
    /// Optional prefix that will limit args to those that begin with the defined prefix.
    ///
    /// Example: The arg -Zdebug=true would become debug=true with a prefix of -Z
    prefix: Option<String>,

    /// Boolean indicating whether invalid flags should be ignored or result in a ConfigError
    ///
    /// Note: the method get_remaining_args always
    ///       returns the invalid args regardless of this boolean
    ignore_invalid: bool,
}

impl CommandLine {
    pub fn with_prefix(s: &str) -> Self {
        CommandLine {
            prefix: Some(s.to_owned()),
            ..CommandLine::default()
        }
    }

    #[must_use]
    pub fn ignore_invalid(mut self, ignore: bool) -> Self {
        self.ignore_invalid = ignore;
        self
    }

    /// Return String iterator of arguments that are invalid.
    pub fn get_remaining_args(self) -> impl Iterator<Item = String> {
        env::args().filter(move |arg| !self.is_valid_arg(arg))
    }

    fn get_prefix(&self) -> String {
        match self.prefix {
            Some(ref prefix) => prefix.to_owned(),
            _ => String::default(),
        }
    }

    fn split_arg<'a>(&'a self, arg: &'a str) -> impl Iterator<Item = String> + 'a {
        arg.splitn(2, '=').map(|s| s.to_owned())
    }

    // An argument is valid if it begins with the optional
    // prefix, the first occurrence of '=' separates two
    // non-empty strings.
    fn is_valid_arg(&self, arg: &str) -> bool {
        let prefix = self.get_prefix();
        if arg.starts_with(&prefix) {
            return self
                .split_arg(&arg[prefix.len()..])
                .map(|s| if s.is_empty() { 3 } else { 1 })
                .sum::<i32>()
                == 2;
        }
        false
    }
}

impl Source for CommandLine {
    fn clone_into_box(&self) -> Box<dyn Source + Send + Sync> {
        Box::new((*self).clone())
    }

    fn collect(&self) -> Result<HashMap<String, Value>, ConfigError> {
        let mut m = HashMap::new();
        let uri = String::from("command-line");

        let prefix_pattern = self.get_prefix();

        for arg in env::args() {
            if !self.is_valid_arg(&arg) {
                if !self.ignore_invalid {
                    return Err(ConfigError::Message(format!(
                        "Invalid command-line arg: '{arg}'"
                    )));
                }

                continue;
            }

            // If arg is valid this can't panic
            let (key, val) = self
                .split_arg(&arg[prefix_pattern.len()..])
                .next_tuple()
                .unwrap();
            m.insert(key.to_lowercase(), Value::new(Some(&uri), val));
        }

        Ok(m)
    }
}
