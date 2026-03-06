use std::process::Command;

fn cargo_bin() -> Command {
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--"]);
    cmd
}

// ---------------------------------------------------------------------------
// Installer-mode (no -c flag) help output
// ---------------------------------------------------------------------------

#[test]
fn test_help_output() {
    let output = cargo_bin()
        .arg("--help")
        .output()
        .expect("failed to run cargo run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{}{}", stdout, String::from_utf8_lossy(&output.stderr));
    // In installer mode we show the ring-cli about text and the init subcommand.
    assert!(
        combined.contains("CLI generator from YAML configurations"),
        "missing about text in:\n{combined}"
    );
    assert!(combined.contains("init"), "missing 'init' subcommand in:\n{combined}");
}

// ---------------------------------------------------------------------------
// Version
// ---------------------------------------------------------------------------

#[test]
fn test_version_output() {
    let output = cargo_bin()
        .arg("--version")
        .output()
        .expect("failed to run cargo run");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let version = env!("CARGO_PKG_VERSION");
    assert!(combined.contains(version), "version {version} not found in:\n{combined}");
}

// ---------------------------------------------------------------------------
// Alias mode: -c <path> is stripped before clap sees it
// Commands are now nested under the config's `name` subcommand.
// ---------------------------------------------------------------------------

#[test]
fn test_load_fixture_config_and_run_command() {
    let output = cargo_bin()
        .args([
            "-c",
            "tests/fixtures/valid_config.yml",
            "test",
            "greet",
            "--name",
            "World",
        ])
        .output()
        .expect("failed to run cargo run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("Hello, World!"),
        "expected 'Hello, World!' in stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn test_multi_step_command() {
    let output = cargo_bin()
        .args(["-c", "tests/fixtures/valid_config.yml", "test", "multi"])
        .output()
        .expect("failed to run cargo run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("step1"),
        "expected 'step1' in stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("step2"),
        "expected 'step2' in stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

// ---------------------------------------------------------------------------
// Alias mode: invalid configs still surface validation errors
// ---------------------------------------------------------------------------

#[test]
fn test_invalid_config_both_cmd_and_subcommands() {
    let output = cargo_bin()
        .args(["-c", "tests/fixtures/invalid_both.yml", "invalid", "bad"])
        .output()
        .expect("failed to run cargo run");
    assert!(
        !output.status.success(),
        "expected failure but process succeeded"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not both"),
        "expected 'not both' in stderr:\n{stderr}"
    );
}

#[test]
fn test_invalid_config_neither_cmd_nor_subcommands() {
    let output = cargo_bin()
        .args(["-c", "tests/fixtures/invalid_neither.yml", "invalid", "bad"])
        .output()
        .expect("failed to run cargo run");
    assert!(
        !output.status.success(),
        "expected failure but process succeeded"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("must be present"),
        "expected 'must be present' in stderr:\n{stderr}"
    );
}

#[test]
fn test_nonexistent_config_path() {
    let output = cargo_bin()
        .args(["-c", "/nonexistent/path/to/config.yml"])
        .output()
        .expect("failed to run cargo run");
    assert!(
        !output.status.success(),
        "expected failure for nonexistent config path"
    );
}

// ---------------------------------------------------------------------------
// Installer mode: init subcommand
// ---------------------------------------------------------------------------

#[test]
fn test_init_creates_file() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let target = dir.path().join("my_config.yml");
    let output = cargo_bin()
        .args([
            "init",
            "--config-path",
            target.to_str().unwrap(),
            "--alias",
            "my-tool",
            "--force",
        ])
        .output()
        .expect("failed to run cargo run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "init failed:\n{stderr}"
    );
    assert!(target.exists(), "expected file to be created at {}", target.display());
    let content = std::fs::read_to_string(&target).unwrap();
    assert!(content.contains("version:"), "missing 'version:' in created file:\n{content}");
    assert!(content.contains("commands:"), "missing 'commands:' in created file:\n{content}");
}

#[test]
fn test_init_existing_config_caches_it() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let target = dir.path().join("existing.yml");
    let yaml = r#"version: "2.0"
name: "existing"
description: "Existing CLI"
commands:
  hello:
    description: "Say hello"
    flags: []
    cmd:
      run:
        - "echo hello"
"#;
    std::fs::write(&target, yaml).unwrap();
    let output = cargo_bin()
        .args(["init", "--config-path", target.to_str().unwrap(), "--alias", "existing-test", "--force"])
        .output()
        .expect("failed to run");
    assert!(
        output.status.success(),
        "init with existing valid config should succeed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// ---------------------------------------------------------------------------
// Alias mode: environment variable substitution
// ---------------------------------------------------------------------------

#[test]
fn test_env_var_replacement() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let config_path = dir.path().join("env_test.yml");
    let yaml = r#"version: "2.0"
name: "envtest"
description: "Env test CLI"
commands:
  greet:
    description: "Greet with env var"
    flags: []
    cmd:
      run:
        - "echo ${{env.RING_TEST_GREETING}}"
"#;
    std::fs::write(&config_path, yaml).unwrap();

    let output = cargo_bin()
        .env("RING_TEST_GREETING", "Howdy")
        .args([
            "-c",
            config_path.to_str().unwrap(),
            "envtest",
            "greet",
        ])
        .output()
        .expect("failed to run cargo run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("Howdy"),
        "expected 'Howdy' in stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

// ---------------------------------------------------------------------------
// Installer mode: alias install helpers
// ---------------------------------------------------------------------------

#[test]
fn test_init_alias_appends_to_shell_config() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let target = dir.path().join("alias_test.yml");
    let output = cargo_bin()
        .args(["init", "--config-path", target.to_str().unwrap(), "--alias", "my-tool", "--force"])
        .output()
        .expect("failed to run cargo run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "init --alias failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(target.exists(), "config file should be created");
    assert!(
        stdout.contains("Created configuration at:"),
        "expected creation message in stdout:\n{stdout}"
    );
}

#[test]
fn test_init_alias_no_duplicate_without_force() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let target1 = dir.path().join("first.yml");
    let output1 = cargo_bin()
        .args(["init", "--config-path", target1.to_str().unwrap(), "--alias", "dup-test", "--force"])
        .output()
        .expect("failed to run cargo run");
    assert!(output1.status.success(), "first init failed");

    // Second init without --force should fail
    let target2 = dir.path().join("second.yml");
    let output2 = cargo_bin()
        .args(["init", "--config-path", target2.to_str().unwrap(), "--alias", "dup-test"])
        .output()
        .expect("failed to run cargo run");
    assert!(!output2.status.success(), "second init should fail without --force");
    let stderr2 = String::from_utf8_lossy(&output2.stderr);
    assert!(
        stderr2.contains("already exists") && stderr2.contains("--force"),
        "expected error mentioning --force:\n{stderr2}"
    );
}

#[test]
fn test_init_alias_overwrite_with_force() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let target1 = dir.path().join("first.yml");
    let output1 = cargo_bin()
        .args(["init", "--config-path", target1.to_str().unwrap(), "--alias", "force-test", "--force"])
        .output()
        .expect("failed to run cargo run");
    assert!(output1.status.success(), "first init failed");

    // Second init with --force should succeed
    let target2 = dir.path().join("second.yml");
    let output2 = cargo_bin()
        .args(["init", "--config-path", target2.to_str().unwrap(), "--alias", "force-test", "--force"])
        .output()
        .expect("failed to run cargo run");
    assert!(
        output2.status.success(),
        "second init with --force should succeed:\n{}",
        String::from_utf8_lossy(&output2.stderr)
    );
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(
        stdout2.contains("is ready!"),
        "expected success message after overwrite:\n{stdout2}"
    );
}

// ---------------------------------------------------------------------------
// Task 9 & 10: help output, color, and edge-case tests
// ---------------------------------------------------------------------------

#[test]
fn test_alias_mode_help_shows_commands() {
    let output = cargo_bin()
        .args(["-c", "tests/fixtures/valid_config.yml", "--help"])
        .output()
        .expect("failed to run");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    // Config has name: "test", so the "test" subcommand should appear in help
    assert!(combined.contains("test"), "missing 'test' config subcommand in help:\n{combined}");
    assert!(combined.contains("refresh-configuration"), "missing 'refresh-configuration':\n{combined}");
}

#[test]
fn test_no_ansi_when_piped() {
    let output = cargo_bin()
        .args(["-c", "tests/fixtures/valid_config.yml", "test", "greet", "--name", "Test"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("\x1b["), "ANSI codes found in piped output:\n{stdout}");
}

#[test]
fn test_no_color_env_disables_ansi() {
    let output = cargo_bin()
        .env("NO_COLOR", "1")
        .env_remove("CARGO_TERM_COLOR")
        .args(["-c", "tests/fixtures/valid_config.yml", "--help"])
        .output()
        .expect("failed to run");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!combined.contains("\x1b["), "ANSI codes found with NO_COLOR=1:\n{combined}");
}

#[test]
fn test_empty_config_shows_help() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let config_path = dir.path().join("empty.yml");
    let yaml = "version: \"2.0\"\nname: \"empty\"\ndescription: \"Empty CLI\"\ncommands: {}\n";
    std::fs::write(&config_path, yaml).unwrap();
    let output = cargo_bin()
        .args(["-c", config_path.to_str().unwrap(), "--help"])
        .output()
        .expect("failed to run");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(combined.contains("empty"), "expected config name in help:\n{combined}");
    assert!(combined.contains("refresh-configuration"), "should still show refresh-configuration:\n{combined}");
}

#[test]
fn test_alias_mode_version() {
    let output = cargo_bin()
        .args(["-c", "tests/fixtures/valid_config.yml", "--version"])
        .output()
        .expect("failed to run");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let version = env!("CARGO_PKG_VERSION");
    assert!(combined.contains(version), "version {version} not found in:\n{combined}");
}

#[test]
fn test_color_flag_never() {
    let output = cargo_bin()
        .args([
            "-c",
            "tests/fixtures/valid_config.yml",
            "--color=never",
            "test",
            "greet",
            "--name",
            "Test",
        ])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, Test!"), "command should still run with --color=never:\n{stdout}");
    assert!(!stdout.contains("\x1b["), "ANSI codes found with --color=never:\n{stdout}");
}

// ---------------------------------------------------------------------------
// --references
// ---------------------------------------------------------------------------

#[test]
fn test_init_with_references_file() {
    let dir = tempfile::TempDir::new().expect("tempdir");

    // Create two config files
    let config_a = dir.path().join("alpha.yml");
    std::fs::write(&config_a, "version: \"2.0\"\nname: \"alpha\"\ndescription: \"Alpha\"\ncommands:\n  hello:\n    description: \"Say hello\"\n    flags: []\n    cmd:\n      run:\n        - \"echo hello\"\n").unwrap();

    let config_b = dir.path().join("beta.yml");
    std::fs::write(&config_b, "version: \"2.0\"\nname: \"beta\"\ndescription: \"Beta\"\ncommands:\n  world:\n    description: \"Say world\"\n    flags: []\n    cmd:\n      run:\n        - \"echo world\"\n").unwrap();

    // Create references file
    let refs = dir.path().join("references.yml");
    std::fs::write(&refs, "configs:\n  - alpha.yml\n  - beta.yml\n").unwrap();

    let output = cargo_bin()
        .args([
            "init",
            "--references",
            refs.to_str().unwrap(),
            "--alias",
            "refs-test",
            "--force",
        ])
        .output()
        .expect("failed to run");
    assert!(
        output.status.success(),
        "init with --references should succeed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify the alias can load both configs
    let help = cargo_bin()
        .args(["--alias-mode", "refs-test", "--help"])
        .output()
        .expect("failed to run help");
    let stdout = String::from_utf8_lossy(&help.stdout);
    assert!(stdout.contains("alpha"), "missing alpha config in help:\n{stdout}");
    assert!(stdout.contains("beta"), "missing beta config in help:\n{stdout}");
}

#[test]
fn test_init_references_missing_config_errors() {
    let dir = tempfile::TempDir::new().expect("tempdir");

    let refs = dir.path().join("references.yml");
    std::fs::write(&refs, "configs:\n  - nonexistent.yml\n").unwrap();

    let output = cargo_bin()
        .args([
            "init",
            "--references",
            refs.to_str().unwrap(),
            "--alias",
            "bad-refs-test",
            "--force",
        ])
        .output()
        .expect("failed to run");
    assert!(
        !output.status.success(),
        "init with missing referenced config should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does not exist"),
        "expected 'does not exist' error:\n{stderr}"
    );
}

// ---------------------------------------------------------------------------
// --check-for-updates / --check-updates
// ---------------------------------------------------------------------------

#[test]
fn test_init_check_for_updates_flag_accepted() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let target = dir.path().join("check_updates.yml");
    let output = cargo_bin()
        .args([
            "init",
            "--config-path",
            target.to_str().unwrap(),
            "--alias",
            "check-test",
            "--check-for-updates",
            "--force",
        ])
        .output()
        .expect("failed to run");
    assert!(
        output.status.success(),
        "init with --check-for-updates should succeed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_check_updates_no_changes_silent() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let config_path = dir.path().join("stable.yml");
    let yaml = "version: \"2.0\"\nname: \"stable\"\ndescription: \"Stable\"\ncommands:\n  hello:\n    description: \"Say hello\"\n    flags: []\n    cmd:\n      run:\n        - \"echo hello\"\n";
    std::fs::write(&config_path, yaml).unwrap();

    // Init to create cache
    let init_output = cargo_bin()
        .args([
            "init",
            "--config-path",
            config_path.to_str().unwrap(),
            "--alias",
            "stable-test",
            "--force",
        ])
        .output()
        .expect("failed to run init");
    assert!(init_output.status.success(), "init failed");

    // Check updates — should produce no output since nothing changed
    let output = cargo_bin()
        .args(["--check-updates", "stable-test"])
        .output()
        .expect("failed to run check-updates");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "check-updates should succeed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !stdout.contains("updates available"),
        "should be silent when no changes:\n{stdout}"
    );
}

#[test]
fn test_check_updates_detects_changes() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let config_path = dir.path().join("changing.yml");
    let yaml = "version: \"2.0\"\nname: \"changing\"\ndescription: \"Changing\"\ncommands:\n  hello:\n    description: \"Say hello\"\n    flags: []\n    cmd:\n      run:\n        - \"echo hello\"\n";
    std::fs::write(&config_path, yaml).unwrap();

    // Init to create cache
    let init_output = cargo_bin()
        .args([
            "init",
            "--config-path",
            config_path.to_str().unwrap(),
            "--alias",
            "changing-test",
            "--force",
        ])
        .output()
        .expect("failed to run init");
    assert!(init_output.status.success(), "init failed");

    // Modify the config file
    let updated_yaml = "version: \"2.0\"\nname: \"changing\"\ndescription: \"Changed!\"\ncommands:\n  hello:\n    description: \"Say hello changed\"\n    flags: []\n    cmd:\n      run:\n        - \"echo hello changed\"\n";
    std::fs::write(&config_path, updated_yaml).unwrap();

    // Check updates — stdin provides 'n' to decline the update prompt
    let output = Command::new("cargo")
        .args(["run", "--", "--check-updates", "changing-test"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(b"n\n").ok();
            }
            child.wait_with_output()
        })
        .expect("failed to run check-updates");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("updates available"),
        "should announce updates when config changed:\n{stdout}"
    );
    assert!(
        stdout.contains("changing"),
        "should mention the config name:\n{stdout}"
    );
}

#[test]
fn test_check_updates_nonexistent_alias_silent() {
    // Should silently succeed even if alias doesn't exist (don't block shell startup)
    let output = cargo_bin()
        .args(["--check-updates", "nonexistent-alias-xyz"])
        .output()
        .expect("failed to run");
    assert!(
        output.status.success(),
        "check-updates for nonexistent alias should succeed silently:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_init_requires_alias() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let target = dir.path().join("no_alias.yml");
    let output = cargo_bin()
        .args(["init", "--config-path", target.to_str().unwrap()])
        .output()
        .expect("failed to run");
    assert!(!output.status.success(), "init without --alias should fail");
}

// ---------------------------------------------------------------------------
// Shell completion tests
// ---------------------------------------------------------------------------

fn init_completions_alias(alias_name: &str) -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let config = dir.path().join("comp.yml");
    std::fs::write(&config, r#"version: "2.0"
name: "myapp"
description: "Test app"
commands:
  deploy:
    description: "Deploy the app"
    flags:
      - name: "env"
        short: "e"
        description: "Target environment"
    cmd:
      run:
        - "echo deploying to ${{env}}"
  status:
    description: "Show status"
    flags: []
    cmd:
      run:
        - "echo ok"
"#).unwrap();
    let output = cargo_bin()
        .args(["init", "--config-path", config.to_str().unwrap(), "--alias", alias_name, "--force"])
        .output()
        .expect("init failed");
    assert!(output.status.success(), "init failed: {}", String::from_utf8_lossy(&output.stderr));
    dir
}

#[test]
fn test_completions_zsh_valid() {
    let _dir = init_completions_alias("comp-zsh");
    let output = cargo_bin()
        .args(["--generate-completions", "zsh", "comp-zsh"])
        .output()
        .expect("failed to generate completions");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("#compdef comp-zsh"), "missing #compdef header:\n{}", &stdout[..200.min(stdout.len())]);
    assert!(stdout.contains("_comp-zsh"), "missing completion function");
    assert!(stdout.contains("compdef _comp-zsh comp-zsh"), "missing compdef binding");
    // Subcommands present
    assert!(stdout.contains("deploy"), "missing deploy subcommand in completions");
    assert!(stdout.contains("status"), "missing status subcommand in completions");
    // Flags present
    assert!(stdout.contains("--env"), "missing --env flag in completions");
    assert!(stdout.contains("-e"), "missing -e short flag in completions");
}

#[test]
fn test_completions_bash_valid() {
    let _dir = init_completions_alias("comp-bash");
    let output = cargo_bin()
        .args(["--generate-completions", "bash", "comp-bash"])
        .output()
        .expect("failed to generate completions");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("complete -F"), "missing complete -F directive");
    assert!(stdout.contains("comp-bash"), "missing alias name in bash completions");
    assert!(stdout.contains("deploy"), "missing deploy in bash completions");
    assert!(stdout.contains("status"), "missing status in bash completions");
}

#[test]
fn test_completions_fish_valid() {
    let _dir = init_completions_alias("comp-fish");
    let output = cargo_bin()
        .args(["--generate-completions", "fish", "comp-fish"])
        .output()
        .expect("failed to generate completions");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("complete -c comp-fish"), "missing complete -c directive");
    assert!(stdout.contains("deploy"), "missing deploy in fish completions");
    assert!(stdout.contains("status"), "missing status in fish completions");
}

#[test]
fn test_completions_powershell_valid() {
    let _dir = init_completions_alias("comp-ps");
    let output = cargo_bin()
        .args(["--generate-completions", "powershell", "comp-ps"])
        .output()
        .expect("failed to generate completions");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("Register-ArgumentCompleter"), "missing Register-ArgumentCompleter");
    assert!(stdout.contains("comp-ps"), "missing alias name in powershell completions");
    assert!(stdout.contains("deploy"), "missing deploy in powershell completions");
    assert!(stdout.contains("status"), "missing status in powershell completions");
}

#[test]
fn test_completions_nested_subcommands() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let config = dir.path().join("nested.yml");
    std::fs::write(&config, r#"version: "2.0"
name: "infra"
description: "Infra tools"
commands:
  db:
    description: "Database operations"
    subcommands:
      migrate:
        description: "Run migrations"
        flags: []
        cmd:
          run:
            - "echo migrating"
      seed:
        description: "Seed database"
        flags:
          - name: "file"
            short: "f"
            description: "Seed file path"
        cmd:
          run:
            - "echo seeding"
"#).unwrap();
    let output = cargo_bin()
        .args(["init", "--config-path", config.to_str().unwrap(), "--alias", "comp-nested", "--force"])
        .output()
        .expect("init failed");
    assert!(output.status.success());

    let comps = cargo_bin()
        .args(["--generate-completions", "zsh", "comp-nested"])
        .output()
        .expect("failed to generate completions");
    let stdout = String::from_utf8_lossy(&comps.stdout);
    assert!(comps.status.success());
    // Nested subcommands present
    assert!(stdout.contains("migrate"), "missing migrate in nested completions");
    assert!(stdout.contains("seed"), "missing seed in nested completions");
    assert!(stdout.contains("--file"), "missing --file flag for seed subcommand");
}

#[test]
fn test_completions_shows_descriptions() {
    let _dir = init_completions_alias("comp-desc");
    let output = cargo_bin()
        .args(["--generate-completions", "zsh", "comp-desc"])
        .output()
        .expect("failed to generate completions");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    // zsh completions include descriptions
    assert!(stdout.contains("Deploy the app"), "missing deploy description in completions");
    assert!(stdout.contains("Show status"), "missing status description in completions");
}

#[test]
fn test_completions_invalid_shell_errors() {
    let _dir = init_completions_alias("comp-bad");
    let output = cargo_bin()
        .args(["--generate-completions", "tcsh", "comp-bad"])
        .output()
        .expect("failed to run");
    assert!(!output.status.success(), "should fail for unknown shell");
}

#[test]
fn test_completions_alias_name_in_output() {
    let _dir = init_completions_alias("my-custom-tool");
    for shell in &["zsh", "bash", "fish", "powershell"] {
        let output = cargo_bin()
            .args(["--generate-completions", shell, "my-custom-tool"])
            .output()
            .unwrap_or_else(|_| panic!("failed for shell {shell}"));
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "{shell} completions failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            stdout.contains("my-custom-tool"),
            "{shell} completions missing alias name:\n{}",
            &stdout[..300.min(stdout.len())]
        );
    }
}

// ---------------------------------------------------------------------------
// Live shell completion tests — source generated scripts in real shells
// ---------------------------------------------------------------------------

fn has_shell(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Generate the completion script for a shell+alias, return as String.
fn generate_completion_script(alias_name: &str, shell: &str) -> String {
    let output = cargo_bin()
        .args(["--generate-completions", shell, alias_name])
        .output()
        .unwrap_or_else(|_| panic!("failed to generate {shell} completions for {alias_name}"));
    assert!(output.status.success(), "generate-completions failed: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).expect("completion script not valid utf-8")
}

/// Extract the bash completion function name from a `complete -F <func> <cmd>` line.
fn extract_bash_func_name(script: &str) -> String {
    for line in script.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("complete -F ") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 3 {
                return parts[2].to_string();
            }
        }
    }
    panic!("no `complete -F` line found in bash completion script");
}

#[test]
fn test_live_shell_bash_top_level() {
    if !has_shell("bash") {
        eprintln!("skipping: bash not available");
        return;
    }
    let _dir = init_completions_alias("livebash");
    let script = generate_completion_script("livebash", "bash");
    let func = extract_bash_func_name(&script);

    let tmp = tempfile::TempDir::new().unwrap();
    let script_path = tmp.path().join("completions.bash");
    std::fs::write(&script_path, &script).unwrap();

    // Top-level: should show config name "myapp" as a subcommand
    let test_cmd = format!(
        r#"source "{path}" 2>&1 && COMP_WORDS=(livebash "") && COMP_CWORD=1 && COMP_LINE="livebash " && COMP_POINT=${{#COMP_LINE}} && {func} "livebash" "" "livebash" 2>/dev/null && printf '%s\n' "${{COMPREPLY[@]}}""#,
        path = script_path.display(),
        func = func,
    );
    let output = Command::new("bash")
        .args(["--norc", "--noprofile", "-c", &test_cmd])
        .output()
        .expect("failed to run bash");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("myapp"), "bash live completions missing 'myapp' config subcommand:\n{stdout}");

    // Second level: "livebash myapp <TAB>" should show deploy, status
    let test_cmd2 = format!(
        r#"source "{path}" 2>&1 && COMP_WORDS=(livebash myapp "") && COMP_CWORD=2 && COMP_LINE="livebash myapp " && COMP_POINT=${{#COMP_LINE}} && {func} "livebash" "" "myapp" 2>/dev/null && printf '%s\n' "${{COMPREPLY[@]}}""#,
        path = script_path.display(),
        func = func,
    );
    let output2 = Command::new("bash")
        .args(["--norc", "--noprofile", "-c", &test_cmd2])
        .output()
        .expect("failed to run bash");
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(stdout2.contains("deploy"), "bash live completions missing 'deploy':\n{stdout2}");
    assert!(stdout2.contains("status"), "bash live completions missing 'status':\n{stdout2}");
}

#[test]
fn test_live_shell_bash_subcommand_flags() {
    if !has_shell("bash") {
        eprintln!("skipping: bash not available");
        return;
    }
    let _dir = init_completions_alias("livebash2");
    let script = generate_completion_script("livebash2", "bash");
    let func = extract_bash_func_name(&script);

    let tmp = tempfile::TempDir::new().unwrap();
    let script_path = tmp.path().join("completions.bash");
    std::fs::write(&script_path, &script).unwrap();

    // Simulate completing "livebash2 myapp deploy --<TAB>"
    let test_cmd = format!(
        r#"source "{path}" 2>&1 && COMP_WORDS=(livebash2 myapp deploy --) && COMP_CWORD=3 && COMP_LINE="livebash2 myapp deploy --" && COMP_POINT=${{#COMP_LINE}} && {func} "livebash2" "--" "deploy" 2>/dev/null && printf '%s\n' "${{COMPREPLY[@]}}""#,
        path = script_path.display(),
        func = func,
    );

    let output = Command::new("bash")
        .args(["--norc", "--noprofile", "-c", &test_cmd])
        .output()
        .expect("failed to run bash");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--env"), "bash live completions missing '--env' flag:\n{stdout}");
}

#[test]
fn test_live_shell_bash_nested_subcommands() {
    if !has_shell("bash") {
        eprintln!("skipping: bash not available");
        return;
    }
    let dir = tempfile::TempDir::new().unwrap();
    let config = dir.path().join("nested.yml");
    std::fs::write(&config, r#"version: "2.0"
name: "infra"
description: "Infra tools"
commands:
  db:
    description: "Database operations"
    subcommands:
      migrate:
        description: "Run migrations"
        flags: []
        cmd:
          run:
            - "echo migrating"
      seed:
        description: "Seed database"
        flags: []
        cmd:
          run:
            - "echo seeding"
"#).unwrap();
    let init = cargo_bin()
        .args(["init", "--config-path", config.to_str().unwrap(), "--alias", "livenested", "--force"])
        .output()
        .expect("init failed");
    assert!(init.status.success());

    let script = generate_completion_script("livenested", "bash");
    let func = extract_bash_func_name(&script);

    let tmp = tempfile::TempDir::new().unwrap();
    let script_path = tmp.path().join("completions.bash");
    std::fs::write(&script_path, &script).unwrap();

    // Simulate completing "livenested infra db <TAB>" (infra is config name, db is the command with subcommands)
    let test_cmd = format!(
        r#"source "{path}" 2>&1 && COMP_WORDS=(livenested infra db "") && COMP_CWORD=3 && COMP_LINE="livenested infra db " && COMP_POINT=${{#COMP_LINE}} && {func} "livenested" "" "db" 2>/dev/null && printf '%s\n' "${{COMPREPLY[@]}}""#,
        path = script_path.display(),
        func = func,
    );

    let output = Command::new("bash")
        .args(["--norc", "--noprofile", "-c", &test_cmd])
        .output()
        .expect("failed to run bash");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("migrate"), "bash nested completions missing 'migrate':\n{stdout}");
    assert!(stdout.contains("seed"), "bash nested completions missing 'seed':\n{stdout}");
}

#[test]
fn test_live_shell_fish_top_level() {
    if !has_shell("fish") {
        eprintln!("skipping: fish not available");
        return;
    }
    let _dir = init_completions_alias("livefish");
    let script = generate_completion_script("livefish", "fish");

    let tmp = tempfile::TempDir::new().unwrap();
    let script_path = tmp.path().join("completions.fish");
    std::fs::write(&script_path, &script).unwrap();

    // fish's `complete -C` prints completions non-interactively
    // Top level: should see config name "myapp", not individual commands
    let test_cmd = format!(
        r#"source "{path}" 2>/dev/null; complete -C "livefish ""#,
        path = script_path.display(),
    );

    let output = Command::new("fish")
        .args(["-c", &test_cmd])
        .output()
        .expect("failed to run fish");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("myapp"), "fish live completions missing 'myapp' config subcommand:\n{stdout}");

    // Second level: "livefish myapp <TAB>" should show deploy, status
    let test_cmd2 = format!(
        r#"source "{path}" 2>/dev/null; complete -C "livefish myapp ""#,
        path = script_path.display(),
    );

    let output2 = Command::new("fish")
        .args(["-c", &test_cmd2])
        .output()
        .expect("failed to run fish");
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(stdout2.contains("deploy"), "fish live completions missing 'deploy':\n{stdout2}");
    assert!(stdout2.contains("status"), "fish live completions missing 'status':\n{stdout2}");
}

#[test]
fn test_live_shell_fish_nested() {
    if !has_shell("fish") {
        eprintln!("skipping: fish not available");
        return;
    }
    let _dir = init_completions_alias("livefish2");
    let script = generate_completion_script("livefish2", "fish");

    let tmp = tempfile::TempDir::new().unwrap();
    let script_path = tmp.path().join("completions.fish");
    std::fs::write(&script_path, &script).unwrap();

    // Complete subcommand flags: "livefish2 myapp deploy --"
    let test_cmd = format!(
        r#"source "{path}" 2>/dev/null; complete -C "livefish2 myapp deploy --""#,
        path = script_path.display(),
    );

    let output = Command::new("fish")
        .args(["-c", &test_cmd])
        .output()
        .expect("failed to run fish");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("env"), "fish live completions missing '--env' flag:\n{stdout}");
}

#[test]
fn test_live_shell_zsh_sources_cleanly() {
    if !has_shell("zsh") {
        eprintln!("skipping: zsh not available");
        return;
    }
    let _dir = init_completions_alias("livezsh");
    let script = generate_completion_script("livezsh", "zsh");

    let tmp = tempfile::TempDir::new().unwrap();
    let script_path = tmp.path().join("completions.zsh");
    std::fs::write(&script_path, &script).unwrap();

    // Verify: sources without error, completion function _livezsh is defined
    let test_cmd = format!(
        r#"autoload -Uz compinit && compinit -u 2>/dev/null && source "{path}" && type _livezsh >/dev/null 2>&1 && echo "FUNCTION_OK""#,
        path = script_path.display(),
    );

    let output = Command::new("zsh")
        .args(["--no-rcs", "-c", &test_cmd])
        .output()
        .expect("failed to run zsh");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "zsh failed to source completion script:\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("FUNCTION_OK"),
        "zsh completion function _livezsh not defined:\nstdout: {stdout}\nstderr: {stderr}"
    );
}

#[test]
fn test_live_shell_powershell_completions() {
    if !has_shell("pwsh") {
        eprintln!("skipping: pwsh not available");
        return;
    }
    let _dir = init_completions_alias("liveps");
    let script = generate_completion_script("liveps", "powershell");

    let tmp = tempfile::TempDir::new().unwrap();
    let script_path = tmp.path().join("completions.ps1");
    std::fs::write(&script_path, &script).unwrap();

    // Dot-source the script, then use TabExpansion2 to get completions
    // Top level: should see config name "myapp", not individual commands
    let test_cmd = format!(
        r#". "{path}"; (TabExpansion2 -inputScript "liveps " -cursorColumn 7).CompletionMatches | ForEach-Object {{ $_.CompletionText }}"#,
        path = script_path.display(),
    );

    let output = Command::new("pwsh")
        .args(["-NoProfile", "-NonInteractive", "-Command", &test_cmd])
        .output()
        .expect("failed to run pwsh");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("myapp"), "pwsh live completions missing 'myapp' config subcommand:\n{stdout}");

    // Second level: "liveps myapp " should show deploy, status
    let test_cmd2 = format!(
        r#". "{path}"; (TabExpansion2 -inputScript "liveps myapp " -cursorColumn 13).CompletionMatches | ForEach-Object {{ $_.CompletionText }}"#,
        path = script_path.display(),
    );

    let output2 = Command::new("pwsh")
        .args(["-NoProfile", "-NonInteractive", "-Command", &test_cmd2])
        .output()
        .expect("failed to run pwsh");
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(stdout2.contains("deploy"), "pwsh live completions missing 'deploy':\n{stdout2}");
    assert!(stdout2.contains("status"), "pwsh live completions missing 'status':\n{stdout2}");
}
