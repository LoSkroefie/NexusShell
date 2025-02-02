mod ssh;
mod sftp;

pub use ssh::SSHPlugin;
pub use sftp::SFTPPlugin;

use async_trait::async_trait;
use super::super::{Command, Environment};
use anyhow::Result;

#[async_trait]
pub trait RemotePlugin: Send + Sync {
    async fn connect(&mut self, host: &str, username: &str, port: u16) -> Result<()>;
    async fn disconnect(&mut self, host: &str) -> Result<()>;
    async fn is_connected(&self, host: &str) -> bool;
}

pub struct RemoteManager {
    ssh: SSHPlugin,
    sftp: SFTPPlugin,
}

impl RemoteManager {
    pub fn new() -> Self {
        RemoteManager {
            ssh: SSHPlugin::new(),
            sftp: SFTPPlugin::new(),
        }
    }

    pub fn get_ssh(&mut self) -> &mut SSHPlugin {
        &mut self.ssh
    }

    pub fn get_sftp(&mut self) -> &mut SFTPPlugin {
        &mut self.sftp
    }
}
