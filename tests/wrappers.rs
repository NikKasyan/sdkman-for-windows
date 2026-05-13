#[cfg(windows)]
mod windows {
    use assert_cmd::cargo::cargo_bin;
    use std::{fs, path::Path, process::Command};
    use tempfile::{Builder, TempDir};

    fn temp_dir() -> TempDir {
        let parent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("wrapper-tests");
        fs::create_dir_all(&parent).unwrap();
        Builder::new().prefix("sdkman-").tempdir_in(parent).unwrap()
    }

    fn prepare_sdk_root() -> TempDir {
        let root = temp_dir();
        let bin = root.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::copy(cargo_bin("sdk"), bin.join("sdk.exe")).unwrap();
        root
    }

    fn repo_path(path: &str) -> String {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(path)
            .display()
            .to_string()
    }

    fn create_fake_command(dir: &Path, command_name: &str, marker: &str) {
        let bin = dir.join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(
            bin.join(format!("{command_name}.cmd")),
            format!("@echo off\r\necho {marker}:%1:%2\r\n"),
        )
        .unwrap();
    }

    fn register_local_sdk(root: &Path, candidate: &str, version: &str, sdk_home: &Path) {
        Command::new(cargo_bin("sdk"))
            .env("SDKMAN_WINDOWS_DIR", root)
            .args(["install", candidate, version, sdk_home.to_str().unwrap()])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                child.stdin.as_mut().unwrap().write_all(b"n\n")?;
                child.wait()
            })
            .unwrap();
    }

    #[test]
    fn powershell_wrapper_env_init_and_clear_do_not_expect_json() {
        let root = prepare_sdk_root();
        let work = temp_dir();
        let script = repo_path("scripts/sdk.ps1");

        let init = Command::new("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                &script,
                "env",
                "init",
            ])
            .env("SDKMAN_WINDOWS_DIR", root.path())
            .current_dir(work.path())
            .output()
            .unwrap();

        assert!(
            init.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&init.stdout),
            String::from_utf8_lossy(&init.stderr)
        );
        assert!(work.path().join(".sdkmanrc").exists());

        let clear = Command::new("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                &script,
                "env",
                "clear",
            ])
            .env("SDKMAN_WINDOWS_DIR", root.path())
            .current_dir(work.path())
            .output()
            .unwrap();

        assert!(
            clear.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&clear.stdout),
            String::from_utf8_lossy(&clear.stderr)
        );
        assert!(!work.path().join(".sdkmanrc").exists());
    }

    #[test]
    fn cmd_wrapper_env_init_and_clear_do_not_execute_output_lines() {
        let root = prepare_sdk_root();
        let work = temp_dir();
        let script = repo_path("scripts/sdk.cmd");

        let init = Command::new("cmd")
            .args(["/C", &script, "env", "init"])
            .env("SDKMAN_WINDOWS_DIR", root.path())
            .current_dir(work.path())
            .output()
            .unwrap();

        assert!(
            init.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&init.stdout),
            String::from_utf8_lossy(&init.stderr)
        );
        assert!(work.path().join(".sdkmanrc").exists());

        let clear = Command::new("cmd")
            .args(["/C", &script, "env", "clear"])
            .env("SDKMAN_WINDOWS_DIR", root.path())
            .current_dir(work.path())
            .output()
            .unwrap();

        assert!(
            clear.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&clear.stdout),
            String::from_utf8_lossy(&clear.stderr)
        );
        assert!(!work.path().join(".sdkmanrc").exists());
    }

    #[test]
    fn powershell_wrapper_default_puts_shims_before_existing_path_commands() {
        let root = prepare_sdk_root();
        let sdk_home = temp_dir();
        let system_home = temp_dir();
        create_fake_command(sdk_home.path(), "sample", "local");
        create_fake_command(system_home.path(), "sample", "system");
        register_local_sdk(root.path(), "sample", "1.0-local", sdk_home.path());

        let script = repo_path("scripts/sdk.ps1");
        let command = format!(
            "& '{}' default sample 1.0-local; sample hello world",
            script.replace('\'', "''")
        );
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &command,
            ])
            .env("SDKMAN_WINDOWS_DIR", root.path())
            .env("PATH", system_home.path().join("bin"))
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("local:hello:world"),
            "stdout:\n{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }

    #[test]
    fn cmd_wrapper_default_puts_shims_before_existing_path_commands() {
        let root = prepare_sdk_root();
        let sdk_home = temp_dir();
        let system_home = temp_dir();
        create_fake_command(sdk_home.path(), "sample", "local");
        create_fake_command(system_home.path(), "sample", "system");
        register_local_sdk(root.path(), "sample", "1.0-local", sdk_home.path());

        let script = repo_path("scripts/sdk.cmd");
        let command = format!("call {script} default sample 1.0-local && sample hello world");
        let output = Command::new("cmd")
            .args(["/C", &command])
            .env("SDKMAN_WINDOWS_DIR", root.path())
            .env("PATH", system_home.path().join("bin"))
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("local:hello:world"),
            "stdout:\n{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}
