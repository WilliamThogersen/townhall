use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Clone, Parser)]
#[command(name = "castle-daemon", about = "Castle agent daemon")]
pub struct Config {
    /// Git repository agents work in (default: current directory).
    #[arg(long, env = "CASTLE_REPO", default_value = ".")]
    pub repo: PathBuf,
    /// Address to bind.
    #[arg(long, env = "CASTLE_BIND", default_value = "127.0.0.1:7777")]
    pub bind: SocketAddr,
}
