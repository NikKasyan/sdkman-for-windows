use assert_cmd::Command;
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
