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
        .args(["init", "--config-path", target.to_str().unwrap(), "--alias", "existing-test"])
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
        .args(["init", "--config-path", target.to_str().unwrap(), "--alias", "my-tool"])
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
fn test_init_alias_no_duplicate() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let target1 = dir.path().join("first.yml");
    let output1 = cargo_bin()
        .args(["init", "--config-path", target1.to_str().unwrap(), "--alias", "dup-test"])
        .output()
        .expect("failed to run cargo run");
    assert!(output1.status.success(), "first init failed");

    let target2 = dir.path().join("second.yml");
    let output2 = cargo_bin()
        .args(["init", "--config-path", target2.to_str().unwrap(), "--alias", "dup-test"])
        .output()
        .expect("failed to run cargo run");
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(
        output2.status.success(),
        "second init failed:\n{}", String::from_utf8_lossy(&output2.stderr)
    );
    assert!(
        stdout2.contains("Created configuration at:"),
        "expected creation message in stdout:\n{stdout2}"
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
