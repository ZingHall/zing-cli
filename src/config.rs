use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub const DEFAULT_API_URL: &str = "https://search.zing.services";
pub const DEFAULT_PLATFORM_USDC_ADDRESS: &str =
    "0x9b1b8ff37a5fdc77141c58ca43a4800a82d6ce91cfaceb7ae7c62c7c80458299";

fn sui_config_dir() -> String {
    std::env::var("SUI_CONFIG_DIR").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "~".to_string());
        format!("{}/.sui/sui_config", home)
    })
}

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

/// Validates that the Sui wallet setup exists and is properly configured.
/// Returns Ok(()) if everything looks good, or Err(help_message) with
/// user-friendly setup instructions.
pub fn validate_setup() -> Result<(), String> {
    let config_dir = sui_config_dir();
    let client_yaml_path = PathBuf::from(&config_dir).join("client.yaml");
    let keystore_path = PathBuf::from(&config_dir).join("sui.keystore");

    if !Path::new(&config_dir).exists() {
        return Err(format_setup_error(
            "Sui config directory not found",
            &config_dir,
            &client_yaml_path,
        ));
    }

    if !client_yaml_path.exists() {
        return Err(format_setup_error(
            "Sui client.yaml not found",
            &config_dir,
            &client_yaml_path,
        ));
    }

    let client_yaml = std::fs::read_to_string(&client_yaml_path)
        .map_err(|e| format!("Cannot read {}: {}", client_yaml_path.display(), e))?;
    let sui_config: SuiClientConfig = serde_yaml::from_str(&client_yaml)
        .map_err(|e| format!("Cannot parse {}: {}", client_yaml_path.display(), e))?;

    if sui_config.active_address.is_none() {
        return Err(format!(
            "No active address set in Sui config.\n\n\
             Run: sui client switch --address <YOUR_ADDRESS>\n\n\
             Diagnostic: no active_address field in {}",
            client_yaml_path.display()
        ));
    }

    if !keystore_path.exists() {
        return Err(format!(
            "Sui keystore not found.\n\n\
             Run: sui client\n\n\
             Diagnostic: {} not found",
            keystore_path.display()
        ));
    }

    Ok(())
}

fn format_setup_error(reason: &str, config_dir: &str, client_yaml_path: &Path) -> String {
    format!(
        "{}. Zing requires a Sui wallet configuration.\n\n\
         Quick setup:\n\
          1. Install Sui CLI:  https://docs.sui.io/guides/developer/getting-started/sui-install\n\
          2. Create wallet:    sui client\n\
          3. Fund wallet:      (at least 0.01 USDC on Sui mainnet)\n\n\
         Then try your command again.\n\n\
         Diagnostic: {} not found\n\
         Config dir:  {}",
        reason,
        client_yaml_path.display(),
        config_dir,
    )
}

pub fn load_config() -> anyhow::Result<ZingConfig> {
    let config_dir = sui_config_dir();

    // 1. Read Sui client.yaml
    let client_yaml_path = PathBuf::from(&config_dir).join("client.yaml");
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
