use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "sdk",
    version,
    about = "SDKMAN for native Windows",
    long_about = "SDKMAN for native Windows\n\nManage SDKMAN-style candidates on Windows. The Rust CLI owns durable state, installs, defaults, and generated shims. The PowerShell and CMD wrappers add shell-local environment updates for commands such as `sdk use` and `sdk env install`.",
    after_help = COMMAND_GUIDE,
    subcommand_required = false
)]
pub struct Args {
    #[arg(long, hide = true)]
    pub emit_env: bool,

    #[arg(long, hide = true)]
    pub emit_cmd: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(
        about = "Create the SDKMAN for Windows directory layout",
        long_about = "Create the SDKMAN for Windows home directory layout.\n\nUse this after copying sdk.exe manually or when you want to create the candidates, shims, cache, and configuration directories before installing SDKs.",
        after_help = "Examples:\n  sdk init\n  sdk config"
    )]
    Init,
    #[command(alias = "ls")]
    #[command(
        about = "List candidates or versions",
        long_about = "List available SDKMAN candidates or versions for one candidate.\n\nWithout a candidate, this shows candidate names such as java or maven. With a candidate, it shows versions and marks installed/current versions where possible. In offline mode, it only shows installed versions.",
        after_help = "Examples:\n  sdk list\n  sdk list java"
    )]
    List {
        #[arg(help = "Candidate to list versions for, for example java or maven")]
        candidate: Option<String>,
        #[arg(
            long,
            value_enum,
            help = "Order versions by vendor then version; default desc (highest first)"
        )]
        order: Option<Order>,
    },
    #[command(alias = "i")]
    #[command(
        about = "Install or register an SDK version",
        long_about = "Install or register an SDK version.\n\nWith only a candidate and version, sdk downloads and installs that version under the SDKMAN for Windows home. With a local path, sdk registers the existing SDK without copying or deleting it later. After install, sdk can set the version as the default.",
        after_help = "Examples:
  sdk install java 21.0.4-tem
  sdk install java 21-local C:\\Tools\\java-21
  sdk install maven 3.9.9 C:\\Tools\\apache-maven-3.9.9"
    )]
    Install {
        #[arg(help = "Candidate name, for example java or maven")]
        candidate: String,
        #[arg(
            help = "Version to install or register. If omitted, the latest available version is used"
        )]
        version: Option<String>,
        #[arg(help = "Existing SDK home to register as a local install")]
        local_path: Option<PathBuf>,
    },
    #[command(alias = "rm")]
    #[command(
        about = "Remove an installed SDK version or unregister a local SDK",
        long_about = "Remove an installed SDK version or unregister a local SDK.\n\nDownloaded SDKs are deleted from the SDKMAN for Windows candidates directory. Locally registered SDKs are only deregistered; the original SDK directory is never deleted. If the removed version was the default, its current link and shims are removed.",
        after_help = "Examples:\n  sdk uninstall java 21.0.4-tem\n  sdk rm java 21-local"
    )]
    Uninstall {
        #[arg(help = "Candidate name, for example java or maven")]
        candidate: String,
        #[arg(
            help = "Installed version to remove or unregister. Omit to choose installed version"
        )]
        version: Option<String>,
    },
    #[command(
        about = "Use a version in the current shell",
        long_about = "Use a version in the current shell only.\n\nThis sets candidate-specific HOME variables and prepends the selected SDK's bin directory to PATH for the active PowerShell or CMD session. It does not change the global default. Invoke this through the installed wrapper so the shell can receive the environment changes.",
        after_help = "Examples:\n  sdk use java 21.0.4-tem\n  sdk use maven 3.9.9"
    )]
    Use {
        #[arg(help = "Candidate name, for example java or maven")]
        candidate: String,
        #[arg(help = "Installed version to use in this shell. Omit to choose installed version")]
        version: Option<String>,
    },
    #[command(alias = "d")]
    #[command(
        about = "Set the default version for a candidate",
        long_about = "Set the default version for a candidate.\n\nThis points the candidate's current link at the selected SDK and regenerates command shims. New shells, and shells where the wrapper has placed the shim directory first on PATH, will resolve commands such as java or mvn through this default.",
        after_help = "Examples:\n  sdk default java 21.0.4-tem\n  sdk default maven 3.9.9"
    )]
    Default {
        #[arg(help = "Candidate name, for example java or maven")]
        candidate: String,
        #[arg(help = "Installed version to make the default. Omit to choose installed version")]
        version: Option<String>,
    },
    #[command(alias = "c")]
    #[command(
        about = "Show active SDK versions",
        long_about = "Show active SDK versions.\n\nWith a candidate, this prints the active home for that candidate. Without a candidate, it prints active homes for all installed candidates. Shell-local selections from `sdk use` take precedence over defaults.",
        after_help = "Examples:\n  sdk current\n  sdk current java"
    )]
    Current {
        #[arg(help = "Candidate to inspect. Omit to show all active candidates")]
        candidate: Option<String>,
    },
    #[command(alias = "h")]
    #[command(
        about = "Print an SDK home directory",
        long_about = "Print an SDK home directory.\n\nWith only a candidate, this prints the active home for that candidate. With a version, it prints that installed version's home. This is useful for scripts that need a stable SDK path.",
        after_help = "Examples:\n  sdk home java\n  sdk home java 21.0.4-tem"
    )]
    Home {
        #[arg(help = "Candidate name, for example java or maven")]
        candidate: String,
        #[arg(help = "Installed version to print. Omit to print the active home")]
        version: Option<String>,
    },
    #[command(
        about = "Manage project-specific .sdkmanrc files",
        long_about = "Manage project-specific .sdkmanrc files.\n\n`env init` creates a .sdkmanrc in the current directory. `env install` reads it and applies the requested versions to the current shell. `env clear` removes the .sdkmanrc. Invoke install through the wrapper so PATH and HOME variables can be updated.",
        after_help = "Examples:\n  sdk env init\n  sdk env install\n  sdk env clear"
    )]
    Env {
        #[arg(help = "Project environment action to run")]
        action: EnvAction,
    },
    #[command(
        about = "Enable or disable offline mode",
        long_about = "Enable or disable offline mode.\n\nOffline mode blocks network-backed commands such as remote install and update. Local installs, defaults, current, home, config, and installed-version listing continue to work.",
        after_help = "Examples:\n  sdk offline enable\n  sdk offline disable"
    )]
    Offline {
        #[arg(help = "Whether to enable or disable offline mode")]
        action: OfflineAction,
    },
    #[command(
        about = "Refresh cached SDKMAN metadata",
        long_about = "Refresh cached SDKMAN metadata.\n\nThis downloads fresh candidate and version metadata so list and install commands can use current SDKMAN catalog information. It requires network access and is blocked in offline mode.",
        after_help = "Examples:\n  sdk update"
    )]
    Update,
    #[command(
        about = "Report that SDK upgrades are not implemented yet",
        long_about = "Report that SDK upgrades are not implemented yet.\n\nSDKMAN for Windows can install, switch, and uninstall versions, but automatic upgrade selection is not implemented yet.",
        after_help = "Examples:\n  sdk upgrade"
    )]
    Upgrade,
    #[command(
        about = "Report that self-update is not implemented yet",
        long_about = "Report that self-update is not implemented yet.\n\nInstall a newer SDKMAN for Windows release by downloading the release artifact and running install.ps1 again.",
        after_help = "Examples:\n  sdk selfupdate"
    )]
    Selfupdate,
    #[command(
        about = "Remove cached downloads, temporary files, or metadata",
        long_about = "Remove cached downloads, temporary files, metadata, or all of them.\n\nUse this when a download cache, extraction work directory, or cached candidate metadata should be rebuilt. It does not remove installed SDK versions.",
        after_help = "Examples:\n  sdk flush tmp\n  sdk flush metadata\n  sdk flush archives\n  sdk flush all"
    )]
    Flush {
        #[arg(help = "Cache area to clear. Defaults to all when omitted")]
        target: Option<FlushTarget>,
    },
    #[command(
        about = "Show or change SDKMAN for Windows configuration",
        long_about = "Show or change SDKMAN for Windows configuration.\n\nWithout a subcommand, this prints the config file path and current values. Use `config set` to update supported SDKMAN-style keys such as auto-answer, timeouts, and offline mode.",
        after_help = "Examples:\n  sdk config\n  sdk config set sdkman_auto_answer true\n  sdk config set sdkman_curl_max_time 12"
    )]
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
    #[command(
        about = "Print version information",
        long_about = "Print SDKMAN for Windows version information.",
        after_help = "Examples:\n  sdk version"
    )]
    Version,
    #[command(hide = true)]
    Complete {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        words: Vec<String>,
    },
}

#[derive(Clone, Debug, ValueEnum)]
pub enum EnvAction {
    #[value(help = "Create a .sdkmanrc file in the current directory")]
    Init,
    #[value(help = "Apply versions from the current directory's .sdkmanrc")]
    Install,
    #[value(help = "Remove the current directory's .sdkmanrc")]
    Clear,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum OfflineAction {
    #[value(help = "Block network-backed commands and use installed/local data")]
    Enable,
    #[value(help = "Allow network-backed commands again")]
    Disable,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum FlushTarget {
    #[value(help = "Clear downloaded archive cache")]
    Archives,
    #[value(help = "Clear temporary extraction files")]
    Tmp,
    #[value(help = "Clear cached candidate and version metadata")]
    Metadata,
    #[value(help = "Clear archives, temporary files, and metadata")]
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Order {
    #[value(help = "Ascending order (lowest first)")]
    Asc,
    #[value(help = "Descending order (highest first)")]
    Desc,
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    #[command(
        about = "Set a configuration key",
        long_about = "Set a configuration key.\n\nSupported keys include sdkman_auto_answer, sdkman_insecure_ssl, sdkman_curl_connect_timeout, sdkman_curl_max_time, sdkman_colour_enable, sdkman_debug_mode, sdkman_healthcheck_enable, sdkman_auto_env, and sdkman_offline_mode.",
        after_help = "Examples:\n  sdk config set sdkman_auto_answer true\n  sdk config set sdkman_curl_max_time 12\n  sdk config set sdkman_offline_mode false"
    )]
    Set {
        #[arg(help = "Configuration key to update")]
        key: String,
        #[arg(help = "New value for the configuration key")]
        value: String,
    },
}

const COMMAND_GUIDE: &str = "Command guide:
  init                Create the SDKMAN for Windows directory layout.
  list   (ls)         Show candidates, or versions for one candidate.
  install (i)         Download an SDK version or register an existing local SDK.
  uninstall (rm)      Remove a downloaded SDK or unregister a local SDK.
  use                 Select a version for the current shell only.
  default (d)         Set the default version and regenerate command shims.
  current (c)         Show the active SDK version or all active versions.
  home    (h)         Print the active or version-specific SDK home.
  env                 Create, apply, or remove project .sdkmanrc files.
  offline             Toggle network-free mode.
  update              Refresh cached SDKMAN candidate/version metadata.
  upgrade             Not implemented yet; use install/default explicitly.
  selfupdate          Not implemented yet; reinstall a release artifact.
  flush               Clear archives, temporary files, metadata, or all caches.
  config              Show or update SDKMAN for Windows configuration.
  version             Print version information.

Run `sdk help <command>` for command-specific details and examples.";

pub fn examples_for(command: &str, subcommand: Option<&str>) -> Option<&'static str> {
    match (command, subcommand) {
        ("init", _) => Some("Examples:\n  sdk init\n  sdk config"),
        ("list" | "ls", _) => Some("Examples:\n  sdk list\n  sdk ls\n  sdk list java"),
        ("install" | "i", _) => Some(
            "Examples:\n  sdk install java 21.0.4-tem\n  sdk i java 21-local C:\\Tools\\java-21\n  sdk install maven 3.9.9 C:\\Tools\\apache-maven-3.9.9",
        ),
        ("uninstall" | "rm", _) => {
            Some("Examples:\n  sdk uninstall java 21.0.4-tem\n  sdk rm java 21-local")
        }
        ("use", _) => Some("Examples:\n  sdk use java 21.0.4-tem\n  sdk use maven 3.9.9"),
        ("default" | "d", _) => {
            Some("Examples:\n  sdk default java 21.0.4-tem\n  sdk d maven 3.9.9")
        }
        ("current" | "c", _) => Some("Examples:\n  sdk current\n  sdk c java"),
        ("home" | "h", _) => Some("Examples:\n  sdk home java\n  sdk h java 21.0.4-tem"),
        ("env", _) => Some("Examples:\n  sdk env init\n  sdk env install\n  sdk env clear"),
        ("offline", _) => Some("Examples:\n  sdk offline enable\n  sdk offline disable"),
        ("update", _) => Some("Examples:\n  sdk update"),
        ("upgrade", _) => Some("Examples:\n  sdk upgrade"),
        ("selfupdate", _) => Some("Examples:\n  sdk selfupdate"),
        ("flush", _) => {
            Some("Examples:\n  sdk flush tmp\n  sdk flush metadata\n  sdk flush archives\n  sdk flush all")
        }
        ("config", Some("set")) => Some(
            "Examples:\n  sdk config set sdkman_auto_answer true\n  sdk config set sdkman_curl_max_time 12\n  sdk config set sdkman_offline_mode false",
        ),
        ("config", _) => Some(
            "Examples:\n  sdk config\n  sdk config set sdkman_auto_answer true\n  sdk config set sdkman_curl_max_time 12",
        ),
        ("version", _) => Some("Examples:\n  sdk version"),
        _ => None,
    }
}
