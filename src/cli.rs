use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "sdk",
    version,
    about = "SDKMAN for native Windows",
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
    #[command(after_help = "Examples:\n  sdk init\n  sdk config")]
    Init,
    #[command(after_help = "Examples:\n  sdk list\n  sdk list java")]
    List { candidate: Option<String> },
    #[command(after_help = "Examples:
  sdk install java 21.0.4-tem
  sdk install java 21-local C:\\Tools\\java-21
  sdk install maven 3.9.9 C:\\Tools\\apache-maven-3.9.9")]
    Install {
        candidate: String,
        version: Option<String>,
        local_path: Option<PathBuf>,
    },
    #[command(alias = "rm")]
    #[command(after_help = "Examples:\n  sdk uninstall java 21.0.4-tem\n  sdk rm java 21-local")]
    Uninstall { candidate: String, version: String },
    #[command(after_help = "Examples:\n  sdk use java 21.0.4-tem\n  sdk use maven 3.9.9")]
    Use { candidate: String, version: String },
    #[command(after_help = "Examples:\n  sdk default java 21.0.4-tem\n  sdk default maven 3.9.9")]
    Default { candidate: String, version: String },
    #[command(after_help = "Examples:\n  sdk current\n  sdk current java")]
    Current { candidate: Option<String> },
    #[command(after_help = "Examples:\n  sdk home java\n  sdk home java 21.0.4-tem")]
    Home {
        candidate: String,
        version: Option<String>,
    },
    #[command(after_help = "Examples:\n  sdk env init\n  sdk env install\n  sdk env clear")]
    Env { action: EnvAction },
    #[command(after_help = "Examples:\n  sdk offline enable\n  sdk offline disable")]
    Offline { action: OfflineAction },
    #[command(after_help = "Examples:\n  sdk update")]
    Update,
    #[command(
        after_help = "Examples:\n  sdk flush tmp\n  sdk flush metadata\n  sdk flush archives\n  sdk flush all"
    )]
    Flush { target: Option<FlushTarget> },
    #[command(
        after_help = "Examples:\n  sdk config\n  sdk config set sdkman_auto_answer true\n  sdk config set sdkman_curl_max_time 12"
    )]
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
    #[command(after_help = "Examples:\n  sdk version")]
    Version,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum EnvAction {
    Init,
    Install,
    Clear,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum OfflineAction {
    Enable,
    Disable,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum FlushTarget {
    Archives,
    Tmp,
    Metadata,
    All,
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    #[command(
        after_help = "Examples:\n  sdk config set sdkman_auto_answer true\n  sdk config set sdkman_curl_max_time 12\n  sdk config set sdkman_offline_mode false"
    )]
    Set { key: String, value: String },
}

pub fn examples_for(command: &str, subcommand: Option<&str>) -> Option<&'static str> {
    match (command, subcommand) {
        ("init", _) => Some("Examples:\n  sdk init\n  sdk config"),
        ("list", _) => Some("Examples:\n  sdk list\n  sdk list java"),
        ("install", _) => Some(
            "Examples:\n  sdk install java 21.0.4-tem\n  sdk install java 21-local C:\\Tools\\java-21\n  sdk install maven 3.9.9 C:\\Tools\\apache-maven-3.9.9",
        ),
        ("uninstall" | "rm", _) => {
            Some("Examples:\n  sdk uninstall java 21.0.4-tem\n  sdk rm java 21-local")
        }
        ("use", _) => Some("Examples:\n  sdk use java 21.0.4-tem\n  sdk use maven 3.9.9"),
        ("default", _) => {
            Some("Examples:\n  sdk default java 21.0.4-tem\n  sdk default maven 3.9.9")
        }
        ("current", _) => Some("Examples:\n  sdk current\n  sdk current java"),
        ("home", _) => Some("Examples:\n  sdk home java\n  sdk home java 21.0.4-tem"),
        ("env", _) => Some("Examples:\n  sdk env init\n  sdk env install\n  sdk env clear"),
        ("offline", _) => Some("Examples:\n  sdk offline enable\n  sdk offline disable"),
        ("update", _) => Some("Examples:\n  sdk update"),
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
