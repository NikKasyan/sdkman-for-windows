use assert_cmd::Command;
#[cfg(windows)]
use std::{fs, process::Command as ProcessCommand};
use tempfile::TempDir;

#[test]
fn version_prints() {
    let temp = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("sdk").unwrap();
    cmd.env("SDKMAN_WINDOWS_DIR", temp.path()).arg("version");
    cmd.assert().success();
}

#[test]
fn init_creates_layout() {
    let temp = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("sdk").unwrap();
    cmd.env("SDKMAN_WINDOWS_DIR", temp.path()).arg("init");
    cmd.assert().success();
    assert!(temp.path().join("candidates").exists());
    assert!(temp.path().join("etc").join("config").exists());
    assert!(temp.path().join("shims").exists());
}

#[cfg(windows)]
#[test]
fn local_install_default_shim_and_uninstall_workflow() {
    let sdkman_home = TempDir::new().unwrap();
    let sdk_home = TempDir::new().unwrap();
    let sdk_bin = sdk_home.path().join("bin");
    fs::create_dir_all(&sdk_bin).unwrap();
    fs::write(
        sdk_bin.join("sample.cmd"),
        "@echo off\r\necho local-sdk:%1:%2\r\n",
    )
    .unwrap();

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", sdkman_home.path())
        .args([
            "install",
            "sample",
            "1.0-local",
            sdk_home.path().to_str().unwrap(),
        ])
        .write_stdin("n\n")
        .assert()
        .success();

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", sdkman_home.path())
        .args(["default", "sample", "1.0-local"])
        .assert()
        .success();

    let shim = sdkman_home.path().join("shims").join("sample.cmd");
    assert!(shim.exists());

    let shim_output = ProcessCommand::new("cmd")
        .args(["/C", shim.to_str().unwrap(), "hello", "world"])
        .output()
        .unwrap();
    assert!(
        shim_output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&shim_output.stdout),
        String::from_utf8_lossy(&shim_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&shim_output.stdout).trim(),
        "local-sdk:hello:world"
    );

    let home_output = Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", sdkman_home.path())
        .args(["home", "sample", "1.0-local"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(
        String::from_utf8_lossy(&home_output).trim(),
        fs::canonicalize(sdk_home.path())
            .unwrap()
            .display()
            .to_string()
    );

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", sdkman_home.path())
        .args(["uninstall", "sample", "1.0-local"])
        .assert()
        .success();

    assert!(sdk_home.path().join("bin").join("sample.cmd").exists());
    assert!(!sdkman_home.path().join("shims").join("sample.cmd").exists());
    assert!(!sdkman_home
        .path()
        .join("candidates")
        .join("sample")
        .join("current")
        .exists());
}
