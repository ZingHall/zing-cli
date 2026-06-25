use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub const DEFAULT_API_URL: &str = "https://search.zing.services";
pub const DEFAULT_PLATFORM_USDC_ADDRESS: &str =
    "0x9b1b8ff37a5fdc77141c58ca43a4800a82d6ce91cfaceb7ae7c62c7c80458299";

fn zing_config_dir() -> String {
    std::env::var("ZING_CONFIG_DIR").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "~".to_string());
        format!("{}/.zing/zing_config", home)
    })
}

#[derive(Debug)]
pub struct ZingConfig {
    pub rpc_url: String,
    pub active_address: sui_sdk_types::Address,
    pub api_base_url: String,
    pub platform_usdc_address: sui_sdk_types::Address,
    pub keystore_path: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ZingClientConfig {
    pub keystore: ZingKeystorePath,
    pub active_address: Option<String>,
    #[serde(default)]
    pub active_env: Option<String>,
    #[serde(default)]
    pub envs: Vec<ZingEnv>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ZingKeystorePath {
    #[serde(rename = "File")]
    pub file: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ZingEnv {
    pub alias: String,
    pub rpc: String,
}

pub fn load_config() -> anyhow::Result<ZingConfig> {
    let config_dir = zing_config_dir();
    let client_yaml_path = PathBuf::from(&config_dir).join("client.yaml");

    if !client_yaml_path.exists() {
        init_config(&config_dir, &client_yaml_path)?;
    }

    let client_yaml = std::fs::read_to_string(&client_yaml_path)
        .map_err(|e| anyhow::anyhow!("Cannot read Zing config at {}: {}", client_yaml_path.display(), e))?;
    let zing_config: ZingClientConfig = serde_yaml::from_str(&client_yaml)?;

    let rpc_url = zing_config
        .active_env
        .as_deref()
        .and_then(|alias| zing_config.envs.iter().find(|e| e.alias == alias))
        .map(|e| e.rpc.clone())
        .unwrap_or_else(|| "https://fullnode.mainnet.sui.io:443".to_string());

    let addr_str = zing_config
        .active_address
        .ok_or_else(|| anyhow::anyhow!("No active_address set in Zing config at {}", client_yaml_path.display()))?;
    let active_address = sui_sdk_types::Address::from_str(&addr_str)?;

    let keystore_path = PathBuf::from(&config_dir).join(&zing_config.keystore.file);

    let api_base_url = std::env::var("ZING_API_URL")
        .unwrap_or_else(|_| DEFAULT_API_URL.to_string());

    let platform_usdc_addr_str = std::env::var("ZING_PLATFORM_USDC_ADDRESS")
        .unwrap_or_else(|_| DEFAULT_PLATFORM_USDC_ADDRESS.to_string());
    let platform_usdc_address = sui_sdk_types::Address::from_str(&platform_usdc_addr_str)?;

    Ok(ZingConfig {
        rpc_url,
        active_address,
        api_base_url,
        platform_usdc_address,
        keystore_path,
    })
}

fn init_config(config_dir: &str, client_yaml_path: &Path) -> anyhow::Result<()> {
    use base64ct::{Base64, Encoding};
    use rand::RngCore;
    use sui_crypto::ed25519::Ed25519PrivateKey;

    std::fs::create_dir_all(config_dir)
        .map_err(|e| anyhow::anyhow!("Cannot create config directory {}: {}", config_dir, e))?;

    let mut key_bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key_bytes);
    let keypair = Ed25519PrivateKey::new(key_bytes);
    let address = keypair.public_key().derive_address();

    let mut raw = vec![0x00u8];
    raw.extend_from_slice(&key_bytes);
    let encoded = Base64::encode_string(&raw);
    let keystore_json = serde_json::to_string_pretty(&vec![encoded])?;
    let keystore_path = PathBuf::from(config_dir).join("zing.keystore");
    std::fs::write(&keystore_path, keystore_json).map_err(|e| {
        anyhow::anyhow!("Cannot write keystore to {}: {}", keystore_path.display(), e)
    })?;

    let config = ZingClientConfig {
        keystore: ZingKeystorePath {
            file: "zing.keystore".to_string(),
        },
        active_address: Some(address.to_string()),
        active_env: Some("mainnet".to_string()),
        envs: vec![ZingEnv {
            alias: "mainnet".to_string(),
            rpc: "https://fullnode.mainnet.sui.io:443".to_string(),
        }],
    };
    let config_yaml = serde_yaml::to_string(&config)?;
    std::fs::write(client_yaml_path, config_yaml).map_err(|e| {
        anyhow::anyhow!("Cannot write config to {}: {}", client_yaml_path.display(), e)
    })?;

    eprintln!(
        "Created new Zing wallet.\n  Address: {}\n\n\
         Fund this address with at least 0.01 USDC on Sui mainnet to use paid search.",
        address
    );

    Ok(())
}
