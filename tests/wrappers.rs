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

    fn ps_quote(value: &str) -> String {
        format!("'{}'", value.replace('\'', "''"))
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

    #[test]
    fn installer_orders_managed_path_entries_and_is_idempotent() {
        let install_dir = temp_dir();
        let existing_dir = temp_dir();
        let sdk_exe = cargo_bin("sdk").display().to_string();
        let script = repo_path("install.ps1");
        let install_text = install_dir.path().display().to_string();
        let script_dir = install_dir.path().join("scripts").display().to_string();
        let shim_dir = install_dir.path().join("shims").display().to_string();
        let bin_dir = install_dir.path().join("bin").display().to_string();
        let existing_text = existing_dir.path().display().to_string();

        let command = format!(
            "$env:Path = {existing}; & {script} -SdkExe {sdk_exe} -InstallDir {install_dir} -PathScope Process -SkipProfileUpdate; & {script} -SdkExe {sdk_exe} -InstallDir {install_dir} -PathScope Process -SkipProfileUpdate; 'PATH_MARKER=' + [Environment]::GetEnvironmentVariable('Path', 'Process')",
            existing = ps_quote(&format!("{existing_text};{bin_dir};{script_dir}\\;{existing_text}")),
            script = ps_quote(&script),
            sdk_exe = ps_quote(&sdk_exe),
            install_dir = ps_quote(&install_text),
        );

        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &command,
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        let path = stdout
            .lines()
            .find_map(|line| line.strip_prefix("PATH_MARKER="))
            .expect("installer test should print PATH_MARKER");
        let entries: Vec<&str> = path.split(';').collect();

        assert_eq!(entries[0], script_dir);
        assert_eq!(entries[1], shim_dir);
        assert_eq!(entries[2], bin_dir);
        assert_eq!(
            entries.iter().filter(|entry| **entry == script_dir).count(),
            1
        );
        assert_eq!(
            entries.iter().filter(|entry| **entry == shim_dir).count(),
            1
        );
        assert_eq!(entries.iter().filter(|entry| **entry == bin_dir).count(), 1);
        assert!(entries.contains(&existing_text.as_str()));
    }

    #[test]
    fn uninstaller_removes_command_integration_but_preserves_data_by_default() {
        let install_dir = temp_dir();
        let existing_dir = temp_dir();
        let sdk_exe = cargo_bin("sdk").display().to_string();
        let install_script = repo_path("install.ps1");
        let uninstall_script = repo_path("uninstall.ps1");
        let install_text = install_dir.path().display().to_string();
        let script_dir = install_dir.path().join("scripts").display().to_string();
        let shim_dir = install_dir.path().join("shims").display().to_string();
        let bin_dir = install_dir.path().join("bin").display().to_string();
        let existing_text = existing_dir.path().display().to_string();

        fs::create_dir_all(install_dir.path().join("candidates").join("java")).unwrap();
        fs::write(
            install_dir
                .path()
                .join("candidates")
                .join("java")
                .join("keep.txt"),
            "sdk",
        )
        .unwrap();

        let command = format!(
            "$env:Path = {existing}; & {install_script} -SdkExe {sdk_exe} -InstallDir {install_dir} -PathScope Process -SkipProfileUpdate; Set-Content -Path {shim_file} -Value '@echo off'; & {uninstall_script} -InstallDir {install_dir} -PathScope Process -SkipProfileUpdate; 'PATH_MARKER=' + [Environment]::GetEnvironmentVariable('Path', 'Process')",
            existing = ps_quote(&existing_text),
            install_script = ps_quote(&install_script),
            uninstall_script = ps_quote(&uninstall_script),
            sdk_exe = ps_quote(&sdk_exe),
            install_dir = ps_quote(&install_text),
            shim_file = ps_quote(&install_dir.path().join("shims").join("sample.cmd").display().to_string()),
        );

        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &command,
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        let path = stdout
            .lines()
            .find_map(|line| line.strip_prefix("PATH_MARKER="))
            .expect("uninstaller test should print PATH_MARKER");
        let entries: Vec<&str> = path.split(';').collect();

        assert_eq!(entries, vec![existing_text.as_str()]);
        assert!(!Path::new(&script_dir).join("sdk.ps1").exists());
        assert!(!Path::new(&script_dir).join("sdk.cmd").exists());
        assert!(!Path::new(&bin_dir).join("sdk.exe").exists());
        assert!(!Path::new(&shim_dir).join("sample.cmd").exists());
        assert!(install_dir
            .path()
            .join("candidates")
            .join("java")
            .join("keep.txt")
            .exists());
    }
}
