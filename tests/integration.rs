use std::process::Command;

fn cargo_bin() -> Command {
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--"]);
    cmd
}

#[test]
fn test_help_output() {
    let output = cargo_bin()
        .arg("--help")
        .output()
        .expect("failed to run cargo run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{}{}", stdout, String::from_utf8_lossy(&output.stderr));
    assert!(combined.contains("Ring CLI Tool"), "missing 'Ring CLI Tool' in:\n{combined}");
    assert!(combined.contains("--quiet"), "missing --quiet in:\n{combined}");
    assert!(combined.contains("--verbose"), "missing --verbose in:\n{combined}");
    assert!(combined.contains("--config"), "missing --config in:\n{combined}");
    assert!(combined.contains("--base-dir"), "missing --base-dir in:\n{combined}");
}

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

#[test]
fn test_load_fixture_config_and_run_command() {
    let output = cargo_bin()
        .args([
            "--config=tests/fixtures/valid_config.yml",
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
        .args(["--config=tests/fixtures/valid_config.yml", "test", "multi"])
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

#[test]
fn test_invalid_config_both_cmd_and_subcommands() {
    let output = cargo_bin()
        .args(["--config=tests/fixtures/invalid_both.yml", "invalid", "bad"])
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
        .args(["--config=tests/fixtures/invalid_neither.yml", "invalid", "bad"])
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
        .arg("--config=/nonexistent/path/to/config.yml")
        .output()
        .expect("failed to run cargo run");
    assert!(
        !output.status.success(),
        "expected failure for nonexistent config path"
    );
}

#[test]
fn test_init_creates_file() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let target = dir.path().join("my_config.yml");
    let output = cargo_bin()
        .args(["init", "--config-path", target.to_str().unwrap()])
        .output()
        .expect("failed to run cargo run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "init failed:\n{stderr}"
    );
    assert!(target.exists(), "expected file to be created at {}", target.display());
    let content = std::fs::read_to_string(&target).unwrap();
    assert!(content.contains("slug:"), "missing 'slug:' in created file:\n{content}");
    assert!(content.contains("commands:"), "missing 'commands:' in created file:\n{content}");
}

#[test]
fn test_init_refuses_overwrite() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let target = dir.path().join("existing.yml");
    std::fs::write(&target, "already here").unwrap();
    let output = cargo_bin()
        .args(["init", "--config-path", target.to_str().unwrap()])
        .output()
        .expect("failed to run cargo run");
    assert!(
        !output.status.success(),
        "expected init to fail when file already exists"
    );
}

#[test]
fn test_env_var_replacement() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let config_path = dir.path().join("env_test.yml");
    let yaml = r#"version: "1.0"
description: "Env test CLI"
slug: "envtest"
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
            &format!("--config={}", config_path.to_str().unwrap()),
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
