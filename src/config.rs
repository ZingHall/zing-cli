use serde::Deserialize;
use std::path::PathBuf;
use std::str::FromStr;

pub const DEFAULT_API_URL: &str = "https://search.zing.services";
pub const DEFAULT_PLATFORM_USDC_ADDRESS: &str =
    "0x9b1b8ff37a5fdc77141c58ca43a4800a82d6ce91cfaceb7ae7c62c7c80458299";

#[derive(Debug)]
pub struct ZingConfig {
    /// Sui RPC URL (mainnet)
    pub rpc_url: String,
    /// Active Sui address (from client.yaml)
    pub active_address: sui_sdk_types::Address,
    /// Indexbind API base URL
    pub api_base_url: String,
    /// Platform USDC address to send payment to
    pub platform_usdc_address: sui_sdk_types::Address,
}

/// Minimal representation of the Sui client.yaml we care about
#[derive(Debug, Deserialize)]
pub struct SuiClientConfig {
    #[allow(dead_code)]
    pub keystore: SuiKeystorePath,
    pub active_address: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SuiKeystorePath {
    #[allow(dead_code)]
    #[serde(rename = "File")]
    pub file: String,
}

pub fn load_config() -> anyhow::Result<ZingConfig> {
    let sui_config_dir = std::env::var("SUI_CONFIG_DIR")
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("HOME must be set");
            format!("{}/.sui/sui_config", home)
        });

    // 1. Read Sui client.yaml
    let client_yaml_path = PathBuf::from(&sui_config_dir).join("client.yaml");
    let client_yaml = std::fs::read_to_string(&client_yaml_path)
        .map_err(|e| anyhow::anyhow!("Cannot read Sui config at {}: {}. Run `sui client` first.", client_yaml_path.display(), e))?;
    let sui_config: SuiClientConfig = serde_yaml::from_str(&client_yaml)?;

    // 2. RPC URL — mainnet
    let rpc_url = "https://fullnode.mainnet.sui.io:443".to_string();

    // 3. Active address from client.yaml
    let addr_str = sui_config.active_address
        .ok_or_else(|| anyhow::anyhow!("No active_address set in Sui config. Run `sui client switch --address <ADDRESS>`"))?;
    let active_address = sui_sdk_types::Address::from_str(&addr_str)?;

    // 4. API base URL — env override or default
    let api_base_url = std::env::var("ZING_API_URL")
        .unwrap_or_else(|_| DEFAULT_API_URL.to_string());

    // 5. Platform USDC address — env override or default
    let platform_usdc_addr_str = std::env::var("ZING_PLATFORM_USDC_ADDRESS")
        .unwrap_or_else(|_| DEFAULT_PLATFORM_USDC_ADDRESS.to_string());
    let platform_usdc_address = sui_sdk_types::Address::from_str(&platform_usdc_addr_str)?;

    Ok(ZingConfig {
        rpc_url,
        active_address,
        api_base_url,
        platform_usdc_address,
    })
}
