use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct KitedConfig {
    #[serde(default)]
    pub identity: IdentityConfig,
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct IdentityConfig {
    /// If set, load keypair from this file; otherwise generate fresh.
    pub keypair_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkConfig {
    /// Multiaddr to listen on. `/ip4/0.0.0.0/tcp/0` binds a random port.
    pub listen_addr: String,
    pub enable_mdns: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_addr: "/ip4/0.0.0.0/tcp/0".into(),
            enable_mdns: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    /// Directory for SQLite receipts and any cached state.
    pub data_dir: PathBuf,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./kited-data"),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub addr: String,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            addr: "127.0.0.1:0".into(),
        }
    }
}
