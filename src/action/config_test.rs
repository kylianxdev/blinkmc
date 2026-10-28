use std::path::PathBuf;

use clap::ArgMatches;

use crate::config::Config;
use crate::util::error::{quit_error, quit_error_msg, ErrorHintsBuilder};

/// Invoke config test command.
pub fn invoke(matches: &ArgMatches) {
    // Get config path, attempt to canonicalize
    let mut path = PathBuf::from(matches.get_one::<String>("config").unwrap());
    if let Some(p) = crate::util::canonicalize(&path) {
        path = p;
    }

    // Ensure it exists
    if !path.is_file() {
        quit_error_msg(
            format!("Config file does not exist at: {}", path.to_str().unwrap()),
            ErrorHintsBuilder::default().build().unwrap(),
        );
    }

    // Try to load config
    let config = match Config::load(path) {
        Ok(config) => config,
        Err(err) => {
            quit_error(
                anyhow!(err).context("Failed to load and parse config"),
                ErrorHintsBuilder::default().verbose(false).build().unwrap(),
            );
        }
    };

    // The command, the server folder and jar, the ports
    if let Err(problems) = config.check() {
        crate::util::error::quit_problems(problems);
    }

    eprintln!("Config loaded successfully!");
}
