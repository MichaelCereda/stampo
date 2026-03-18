mod cache;
mod cli;
mod config;
mod errors;
mod init;
mod models;
mod openapi;
mod refresh;
mod shell;
mod style;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Handle completion generation (called by shell hooks installed during init)
    if let Some(pos) = args.iter().position(|a| a == "--generate-completions") {
        let shell_name = args
            .get(pos + 1)
            .ok_or_else(|| anyhow::anyhow!("Missing shell name after --generate-completions"))?;
        let alias_name = args
            .get(pos + 2)
            .ok_or_else(|| anyhow::anyhow!("Missing alias name after --generate-completions"))?;

        let shell: clap_complete::Shell = shell_name
            .parse()
            .map_err(|_| anyhow::anyhow!("Unknown shell: {shell_name}"))?;

        let (config_contents, _metadata) = cache::load_trusted_configs(alias_name)?;
        let configs: Vec<models::Configuration> = config_contents
            .iter()
            .map(|c| {
                serde_saphyr::from_str(c)
                    .map_err(|e| anyhow::anyhow!("Cached config invalid: {e}"))
            })
            .collect::<Result<_, _>>()?;

        let mut cmd = cli::build_cli(&configs, alias_name, _metadata.description.as_deref());
        clap_complete::generate(shell, &mut cmd, alias_name.as_str(), &mut std::io::stdout());
        return Ok(());
    }

    // Handle update check (called by shell startup hook installed via --check-for-updates)
    if let Some(pos) = args.iter().position(|a| a == "--check-updates") {
        let alias_name = args
            .get(pos + 1)
            .ok_or_else(|| anyhow::anyhow!("Missing alias name after --check-updates"))?;
        return refresh::handle_check_updates(alias_name, false);
    }

    // Detect alias mode (--alias-mode <name>)
    let alias_mode = args
        .iter()
        .position(|a| a == "--alias-mode")
        .and_then(|i| args.get(i + 1).cloned());

    // Also support legacy -c / --config for backwards compat
    let config_path = args
        .iter()
        .find(|arg| arg.starts_with("--config="))
        .and_then(|arg| arg.split('=').nth(1).map(String::from))
        .or_else(|| {
            args.iter()
                .position(|a| a == "-c" || a == "--config")
                .and_then(|i| args.get(i + 1).cloned())
        });

    if let Some(ref alias_name) = alias_mode {
        // ALIAS MODE: load all configs from cache for this alias
        let (config_contents, _metadata) = cache::load_trusted_configs(alias_name)?;
        let configs: Vec<models::Configuration> = config_contents
            .iter()
            .zip(_metadata.configs.iter())
            .map(|(c, entry)| {
                let mut config: models::Configuration = serde_saphyr::from_str(c)
                    .map_err(|e| anyhow::anyhow!("Invalid cached config: {e}"))?;
                // Resolve relative base_dir against the original config file's directory
                init::resolve_base_dir(&mut config, &entry.source_path);
                Ok::<_, anyhow::Error>(config)
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Strip --alias-mode and its value from args, replace argv[0] with alias name
        let clap_args: Vec<String> = {
            let mut out = Vec::with_capacity(args.len());
            let mut skip_next = false;
            let mut first = true;
            for arg in &args {
                if first {
                    out.push(alias_name.clone());
                    first = false;
                    continue;
                }
                if skip_next {
                    skip_next = false;
                    continue;
                }
                if arg == "--alias-mode" {
                    skip_next = true;
                    continue;
                }
                out.push(arg.clone());
            }
            out
        };

        let matches = cli::build_cli(&configs, alias_name, _metadata.description.as_deref()).get_matches_from(clap_args);

        // Initialize color mode
        let color_str = matches.get_one::<String>("color").map(|s| s.as_str()).unwrap_or("auto");
        style::init(match color_str {
            "always" => style::ColorMode::Always,
            "never" => style::ColorMode::Never,
            _ => style::ColorMode::Auto,
        });

        let is_quiet = matches.get_flag("quiet");
        let is_verbose = matches.get_flag("verbose");

        // Display banner if configured and not in quiet mode
        if !is_quiet {
            if let Some(ref banner) = _metadata.banner {
                eprintln!("{}", banner);
            } else {
                for config in &configs {
                    if let Some(ref banner) = config.banner {
                        eprintln!("{}", banner);
                    }
                }
            }
        }

        // Handle refresh-configuration
        if let Some(refresh_matches) = matches.subcommand_matches("refresh-configuration") {
            let refresh_yes = refresh_matches.get_flag("yes");
            return refresh::handle_refresh_configuration(alias_name, refresh_yes);
        }

        // Dispatch: match config name subcommand, then command within that config
        for config in &configs {
            if let Some(config_matches) = matches.subcommand_matches(&config.name) {
                for (cmd_name, cmd) in &config.commands {
                    if let Some(cmd_matches) = config_matches.subcommand_matches(cmd_name)
                        && let Err(e) = cli::execute_command(cmd, cmd_matches, is_verbose, config.base_dir.as_deref()) {
                            if !is_quiet {
                                eprintln!("{}", style::error(&e.to_string()));
                            }
                            std::process::exit(1);
                        }
                }
            }
        }
    } else if let Some(ref path) = config_path {
        // LEGACY SINGLE-CONFIG MODE: -c <path>
        let mut config = config::load_configuration(path)?;
        init::resolve_base_dir(&mut config, path);
        let configs = vec![config];

        // Strip the config flag (and its value) from args before clap parses them
        let clap_args: Vec<String> = {
            let mut out = Vec::with_capacity(args.len());
            let mut skip_next = false;
            for arg in &args {
                if skip_next {
                    skip_next = false;
                    continue;
                }
                if arg.starts_with("--config=") {
                    continue;
                }
                if arg == "-c" || arg == "--config" {
                    skip_next = true;
                    continue;
                }
                out.push(arg.clone());
            }
            out
        };

        let matches = cli::build_cli(&configs, "ring-cli", None).get_matches_from(clap_args);

        // Initialize color mode
        let color_str = matches.get_one::<String>("color").map(|s| s.as_str()).unwrap_or("auto");
        style::init(match color_str {
            "always" => style::ColorMode::Always,
            "never" => style::ColorMode::Never,
            _ => style::ColorMode::Auto,
        });

        let is_quiet = matches.get_flag("quiet");
        let is_verbose = matches.get_flag("verbose");

        // Handle refresh-configuration
        if matches.subcommand_matches("refresh-configuration").is_some() {
            // In legacy mode we don't have an alias name — inform user to reinitialise
            eprintln!("{}", style::error("refresh-configuration requires --alias-mode. Please re-run 'ring-cli init' to update your alias."));
            std::process::exit(1);
        }

        // Dispatch: commands are nested under config.name subcommand
        for config in &configs {
            if let Some(config_matches) = matches.subcommand_matches(&config.name) {
                for (cmd_name, cmd) in &config.commands {
                    if let Some(cmd_matches) = config_matches.subcommand_matches(cmd_name)
                        && let Err(e) = cli::execute_command(cmd, cmd_matches, is_verbose, config.base_dir.as_deref()) {
                            if !is_quiet {
                                eprintln!("{}", style::error(&e.to_string()));
                            }
                            std::process::exit(1);
                        }
                }
            }
        }
    } else {
        // INSTALLER MODE
        let matches = cli::build_ring_cli().get_matches();

        if let Some(init_matches) = matches.subcommand_matches("init") {
            let config_paths = init_matches.get_many::<String>("config-path");
            let references = init_matches.get_one::<String>("references");
            let alias = init_matches.get_one::<String>("alias");
            let warn_only = init_matches.get_flag("warn-only-on-conflict");
            let check_for_updates = init_matches.get_flag("check-for-updates");
            let force = init_matches.get_flag("force");
            let yes = init_matches.get_flag("yes");
            let verbose = init_matches.get_flag("verbose");
            let description = init_matches.get_one::<String>("description");
            return init::handle_init(config_paths, references, alias, warn_only, check_for_updates, force, yes, verbose, description);
        }
    }

    Ok(())
}
