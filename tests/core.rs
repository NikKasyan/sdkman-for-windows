use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
#[cfg(windows)]
use std::process::Command as ProcessCommand;
use std::{fs, path::Path};
use tempfile::TempDir;

#[test]
fn version_prints() {
    let temp = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("sdk").unwrap();
    cmd.env("SDKMAN_WINDOWS_DIR", temp.path()).arg("version");
    cmd.assert().success();
}

#[test]
fn no_args_prints_clap_help() {
    let temp = TempDir::new().unwrap();
    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", temp.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("SDKMAN for native Windows"))
        .stdout(predicates::str::contains("Usage: sdk"))
        .stdout(predicates::str::contains("Examples:").not());
}

#[test]
fn help_prints_command_specific_help() {
    let temp = TempDir::new().unwrap();
    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", temp.path())
        .args(["help", "install"])
        .assert()
        .success()
        .stdout(predicates::str::contains("install <CANDIDATE>"))
        .stdout(predicates::str::contains("<CANDIDATE>"))
        .stdout(predicates::str::contains("Examples:"))
        .stdout(predicates::str::contains("sdk install java 21.0.4-tem"));

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", temp.path())
        .args(["help", "config"])
        .assert()
        .success()
        .stdout(predicates::str::contains("config"))
        .stdout(predicates::str::contains("set"))
        .stdout(predicates::str::contains(
            "sdk config set sdkman_curl_max_time 12",
        ));

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", temp.path())
        .args(["help", "config", "set"])
        .assert()
        .success()
        .stdout(predicates::str::contains("sdkman_auto_answer"))
        .stdout(predicates::str::contains("Examples:"))
        .stdout(predicates::str::contains(
            "sdk config set sdkman_offline_mode false",
        ));
}

#[test]
fn invalid_command_usage_prints_error_and_examples() {
    let temp = TempDir::new().unwrap();
    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", temp.path())
        .arg("install")
        .assert()
        .failure()
        .stderr(predicates::str::contains("required").or(predicates::str::contains("missing")))
        .stderr(predicates::str::contains("Stop!").not())
        .stderr(predicates::str::contains("Examples:"))
        .stderr(predicates::str::contains("sdk install java 21.0.4-tem"));
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

#[test]
fn config_prints_default_values() {
    let temp = TempDir::new().unwrap();
    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", temp.path())
        .arg("config")
        .assert()
        .success()
        .stdout(predicates::str::contains("Config:"))
        .stdout(predicates::str::contains("sdkman_auto_answer=false"))
        .stdout(predicates::str::contains("sdkman_curl_max_time=60"))
        .stdout(predicates::str::contains("sdkman_offline_mode=false"));
}

#[test]
fn config_prints_custom_values() {
    let temp = TempDir::new().unwrap();
    let config_dir = temp.path().join("etc");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config"),
        "sdkman_auto_answer=true\nsdkman_curl_max_time=9\nsdkman_offline_mode=true\n",
    )
    .unwrap();

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", temp.path())
        .arg("config")
        .assert()
        .success()
        .stdout(predicates::str::contains("sdkman_auto_answer=true"))
        .stdout(predicates::str::contains("sdkman_curl_max_time=9"))
        .stdout(predicates::str::contains("sdkman_offline_mode=true"));
}

#[test]
fn config_set_updates_boolean_value() {
    let temp = TempDir::new().unwrap();

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", temp.path())
        .args(["config", "set", "sdkman_auto_answer", "true"])
        .assert()
        .success()
        .stdout(predicates::str::contains("sdkman_auto_answer=true"));

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", temp.path())
        .arg("config")
        .assert()
        .success()
        .stdout(predicates::str::contains("sdkman_auto_answer=true"));
}

#[test]
fn config_set_updates_integer_value() {
    let temp = TempDir::new().unwrap();

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", temp.path())
        .args(["config", "set", "sdkman_curl_max_time", "12"])
        .assert()
        .success()
        .stdout(predicates::str::contains("sdkman_curl_max_time=12"));

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", temp.path())
        .arg("config")
        .assert()
        .success()
        .stdout(predicates::str::contains("sdkman_curl_max_time=12"));
}

#[test]
fn config_set_rejects_unknown_key() {
    let temp = TempDir::new().unwrap();

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", temp.path())
        .args(["config", "set", "sdkman_missing", "true"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "unknown config key: sdkman_missing",
        ));
}

#[test]
fn config_set_rejects_invalid_value() {
    let temp = TempDir::new().unwrap();

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", temp.path())
        .args(["config", "set", "sdkman_auto_answer", "yes"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "sdkman_auto_answer expects true or false",
        ));

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", temp.path())
        .args(["config", "set", "sdkman_curl_max_time", "slow"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "sdkman_curl_max_time expects a non-negative integer",
        ));
}

#[test]
fn complete_use_versions_from_installed_versions() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join("candidates").join("java").join("21-local")).unwrap();
    fs::create_dir_all(
        temp.path()
            .join("candidates")
            .join("java")
            .join("25.0.3-tem"),
    )
    .unwrap();

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", temp.path())
        .args(["complete", "use", "java", "2"])
        .assert()
        .success()
        .stdout(predicates::str::contains("21-local"))
        .stdout(predicates::str::contains("25.0.3-tem"));
}

#[test]
fn complete_install_versions_uses_offline_metadata_cache() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join("etc")).unwrap();
    fs::create_dir_all(temp.path().join("var").join("metadata")).unwrap();
    fs::write(
        temp.path().join("etc").join("config"),
        "sdkman_offline_mode=true\n",
    )
    .unwrap();
    fs::write(
        temp.path()
            .join("var")
            .join("metadata")
            .join("java-versions.txt"),
        "25.0.3-tem,21.0.11-tem,17.0.19-tem",
    )
    .unwrap();

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", temp.path())
        .args(["complete", "install", "java", "25"])
        .assert()
        .success()
        .stdout(predicates::str::contains("25.0.3-tem"))
        .stdout(predicates::str::contains("21.0.11-tem").not());
}

#[cfg(windows)]
fn create_fake_sdk(command_name: &str) -> TempDir {
    create_fake_sdk_with_marker(command_name, "local-sdk")
}

#[cfg(windows)]
fn create_fake_sdk_with_marker(command_name: &str, marker: &str) -> TempDir {
    let sdk_home = TempDir::new().unwrap();
    let sdk_bin = sdk_home.path().join("bin");
    fs::create_dir_all(&sdk_bin).unwrap();
    fs::write(
        sdk_bin.join(format!("{command_name}.cmd")),
        format!("@echo off\r\necho {marker}:%1:%2\r\n"),
    )
    .unwrap();
    sdk_home
}

#[cfg(windows)]
fn register_local_sdk(sdkman_home: &Path, candidate: &str, version: &str, sdk_home: &Path) {
    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", sdkman_home)
        .args(["install", candidate, version, sdk_home.to_str().unwrap()])
        .write_stdin("n\n")
        .assert()
        .success();
}

#[cfg(windows)]
#[test]
fn local_install_default_shim_and_uninstall_workflow() {
    let sdkman_home = TempDir::new().unwrap();
    let sdk_home = create_fake_sdk("sample");

    register_local_sdk(sdkman_home.path(), "sample", "1.0-local", sdk_home.path());

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", sdkman_home.path())
        .args(["default", "sample", "1.0-local"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Default sample version set to 1.0-local.",
        ))
        .stdout(predicates::str::contains("Junction created").not());

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", sdkman_home.path())
        .args(["offline", "enable"])
        .assert()
        .success();

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", sdkman_home.path())
        .args(["list", "sample"])
        .assert()
        .success()
        .stdout(predicates::str::contains("> * 1.0-local"));

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
    let home_text = String::from_utf8_lossy(&home_output);
    assert!(!home_text.contains(r"\\?\"));
    assert!(home_text
        .trim()
        .ends_with(sdk_home.path().file_name().unwrap().to_str().unwrap()));

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

#[cfg(windows)]
#[test]
fn sdkmanrc_env_install_emits_powershell_json_and_cmd_commands() {
    let sdkman_home = TempDir::new().unwrap();
    let work = TempDir::new().unwrap();
    let sdk_home = create_fake_sdk("java");
    register_local_sdk(sdkman_home.path(), "java", "21-local", sdk_home.path());
    fs::write(work.path().join(".sdkmanrc"), "java=21-local\n").unwrap();

    let powershell_output = Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", sdkman_home.path())
        .current_dir(work.path())
        .args(["--emit-env", "env", "install"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let powershell_text = String::from_utf8_lossy(&powershell_output);
    let json = powershell_text
        .lines()
        .find_map(|line| line.strip_prefix("__SDKMAN_ENV_JSON__"))
        .unwrap();
    let update: serde_json::Value = serde_json::from_str(json).unwrap();
    let sdk_home = fs::canonicalize(sdk_home.path()).unwrap();
    let sdk_home_text = sdk_home.display().to_string();
    let sdk_bin_text = sdk_home.join("bin").display().to_string();

    assert_eq!(update["set"]["JAVA_HOME"], sdk_home_text);
    assert_eq!(update["set"]["SDKMAN_JAVA_HOME"], sdk_home_text);
    assert_eq!(update["prepend_path"][0], sdk_bin_text);
    assert_eq!(update["message"], "Applied .sdkmanrc");

    let cmd_output = Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", sdkman_home.path())
        .current_dir(work.path())
        .args(["--emit-cmd", "env", "install"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let cmd_text = String::from_utf8_lossy(&cmd_output);
    assert!(cmd_text.contains(&format!("set \"JAVA_HOME={sdk_home_text}\"")));
    assert!(cmd_text.contains(&format!("set \"SDKMAN_JAVA_HOME={sdk_home_text}\"")));
    assert!(cmd_text.contains(&format!("set \"PATH={sdk_bin_text};%PATH%\"")));
    assert!(cmd_text.contains("echo Applied .sdkmanrc"));
}

#[cfg(windows)]
#[test]
fn sdkmanrc_env_install_fails_when_version_is_missing() {
    let sdkman_home = TempDir::new().unwrap();
    let work = TempDir::new().unwrap();
    fs::write(work.path().join(".sdkmanrc"), "java=missing-local\n").unwrap();

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", sdkman_home.path())
        .current_dir(work.path())
        .args(["--emit-env", "env", "install"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "java missing-local is not installed",
        ));
}

#[cfg(windows)]
#[test]
fn offline_mode_allows_local_workflows_and_blocks_network_workflows() {
    let sdkman_home = TempDir::new().unwrap();
    let sdk_home = create_fake_sdk("sample");
    register_local_sdk(sdkman_home.path(), "sample", "1.0-local", sdk_home.path());

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", sdkman_home.path())
        .args(["offline", "enable"])
        .assert()
        .success();

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", sdkman_home.path())
        .args(["list", "sample"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Offline Mode: only showing installed sample versions",
        ))
        .stdout(predicates::str::contains("* 1.0-local"));

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", sdkman_home.path())
        .args(["install", "java", "21-remote"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "install requires network while offline mode is enabled",
        ));

    let offline_sdk_home = create_fake_sdk("offline");
    register_local_sdk(
        sdkman_home.path(),
        "offline",
        "1.0-local",
        offline_sdk_home.path(),
    );

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", sdkman_home.path())
        .args(["update"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "update requires network while offline mode is enabled",
        ));
}

#[test]
fn upgrade_reports_nothing_to_do_when_no_candidates_installed() {
    let temp = TempDir::new().unwrap();

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", temp.path())
        .arg("upgrade")
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "No installed candidates to check.",
        ));
}

#[test]
fn upgrade_fails_in_offline_mode() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join("etc")).unwrap();
    fs::write(
        temp.path().join("etc").join("config"),
        "sdkman_offline_mode=true\n",
    )
    .unwrap();

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", temp.path())
        .arg("upgrade")
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "upgrade requires network while offline mode is enabled",
        ));
}

#[test]
fn selfupdate_fails_in_offline_mode() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join("etc")).unwrap();
    fs::write(
        temp.path().join("etc").join("config"),
        "sdkman_offline_mode=true\n",
    )
    .unwrap();

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", temp.path())
        .arg("selfupdate")
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "selfupdate requires network while offline mode is enabled",
        ));
}

#[cfg(windows)]
#[test]
fn default_regenerates_shims_idempotently_and_removes_stale_shims() {
    let sdkman_home = TempDir::new().unwrap();
    let first_home = create_fake_sdk_with_marker("sample", "first");
    let second_home = create_fake_sdk_with_marker("sample", "second");

    register_local_sdk(sdkman_home.path(), "sample", "1.0-local", first_home.path());
    register_local_sdk(
        sdkman_home.path(),
        "sample",
        "2.0-local",
        second_home.path(),
    );

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", sdkman_home.path())
        .args(["default", "sample", "1.0-local"])
        .assert()
        .success();

    let shim = sdkman_home.path().join("shims").join("sample.cmd");
    let first_shim = fs::read_to_string(&shim).unwrap();

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", sdkman_home.path())
        .args(["default", "sample", "1.0-local"])
        .assert()
        .success();

    assert_eq!(fs::read_to_string(&shim).unwrap(), first_shim);

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", sdkman_home.path())
        .args(["default", "sample", "2.0-local"])
        .assert()
        .success();

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
        "second:hello:world"
    );

    Command::cargo_bin("sdk")
        .unwrap()
        .env("SDKMAN_WINDOWS_DIR", sdkman_home.path())
        .args(["uninstall", "sample", "2.0-local"])
        .assert()
        .success();

    assert!(!shim.exists());
}
