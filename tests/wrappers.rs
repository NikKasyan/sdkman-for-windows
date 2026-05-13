#[cfg(windows)]
mod windows {
    use assert_cmd::cargo::cargo_bin;
    use std::{fs, path::Path, process::Command};
    use tempfile::{Builder, TempDir};

    fn temp_dir() -> TempDir {
        let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/wrapper-tests");
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
}
