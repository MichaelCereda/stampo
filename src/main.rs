mod cli;
mod models;
mod utils;

use clap::ArgMatches;

fn main() {
    let config_path = std::env::args()
        .find(|arg| arg.starts_with("--config=") || arg == "-c")
        .and_then(|arg| arg.split('=').nth(1).map(String::from));

    let configurations = utils::load_configurations(config_path.as_deref()).unwrap_or_else(|e| {
        eprintln!("Error loading configurations: {}", e);
        std::process::exit(1);
    });

    let matches = cli::build_cli_from_configs(&configurations).get_matches();

    let is_quiet = matches.is_present("quiet");
    let is_verbose = matches.is_present("verbose");
    let base_dir = matches.value_of("base_dir");
    for config in &configurations {
        if let Some(submatches) = matches.subcommand_matches(&config.slug) {
            for (cmd_name, cmd) in &config.commands {
                if let Some(cmd_matches) = submatches.subcommand_matches(cmd_name) {
                    if let Err(e) = cli::execute_command(cmd, cmd_matches, is_verbose, base_dir) {
                        if !is_quiet {
                            eprintln!("Error: {}", e);
                        }
                        std::process::exit(1);
                    }
                }
            }
        }
    }
}
