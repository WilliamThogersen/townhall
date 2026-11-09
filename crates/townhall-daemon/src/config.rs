use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Clone, Parser)]
#[command(
    name = "townhall-daemon",
    about = "Townhall agent daemon",
    version = townhall_core::version::VERSION
)]
pub struct Config {
    /// Git repository agents work in (default: current directory).
    #[arg(long, env = "TOWNHALL_REPO", default_value = ".")]
    pub repo: PathBuf,
    /// Address to bind.
    #[arg(long, env = "TOWNHALL_BIND", default_value = "127.0.0.1:7777")]
    pub bind: SocketAddr,
}
