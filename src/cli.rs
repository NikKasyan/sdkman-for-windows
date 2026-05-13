use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "sdk", version, about = "SDKMAN for native Windows")]
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
    Init,
    List {
        candidate: Option<String>,
    },
    Install {
        candidate: String,
        version: Option<String>,
        local_path: Option<PathBuf>,
    },
    #[command(alias = "rm")]
    Uninstall {
        candidate: String,
        version: String,
    },
    Use {
        candidate: String,
        version: String,
    },
    Default {
        candidate: String,
        version: String,
    },
    Current {
        candidate: Option<String>,
    },
    Home {
        candidate: String,
        version: Option<String>,
    },
    Env {
        action: EnvAction,
    },
    Offline {
        action: OfflineAction,
    },
    Update,
    Flush {
        target: Option<FlushTarget>,
    },
    Config,
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
