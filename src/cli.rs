use clap::{Arg, Command};

/// The clap app for CLI argument parsing.
pub fn app() -> Command {
    Command::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .subcommand(
            Command::new("start")
                .alias("run")
                .about("Start lazymc and server (default)"),
        )
        .subcommand(
            Command::new("init")
                .about("Write a blink.toml for the server in this folder")
                .arg(Arg::new("heap").long("heap").value_name("SIZE").help("Maximum Java heap, e.g. 6G (default: 60% of the RAM, at most 8G)").num_args(1))
                .arg(Arg::new("sleep").long("sleep").value_name("SECONDS").default_value("30").value_parser(clap::value_parser!(u32)).help("Save and freeze this many seconds after the last player left"))
                .arg(Arg::new("threads").long("threads").value_name("N").default_value("1").value_parser(clap::value_parser!(usize)).help("Threads for the players' traffic"))
                .arg(Arg::new("force").long("force").action(clap::ArgAction::SetTrue).help("Overwrite an existing blink.toml")),
        )
        .subcommand(
            Command::new("config")
                .alias("cfg")
                .about("Config actions")
                .arg_required_else_help(true)
                .subcommand_required(true)
                .subcommand(
                    Command::new("generate")
                        .alias("gen")
                        .about("Generate config"),
                )
                .subcommand(Command::new("test").about("Test config")),
        )
        .arg(
            Arg::new("config")
                .short('c')
                .alias("cfg")
                .long("config")
                .global(true)
                .value_name("FILE")
                .default_value(crate::config::CONFIG_FILE)
                .help("Use config file")
                .num_args(1),
        )
}
