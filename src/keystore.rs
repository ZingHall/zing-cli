use sui_crypto::ed25519::Ed25519PrivateKey;
use sui_sdk_types::Address;
use std::path::Path;
use base64ct::Encoding;

/// Load the Ed25519 private key and derive its address from the zing keystore.
///
/// The keystore is a JSON array of base64-encoded `flag || privkey` (33 bytes).
/// flag 0x00 = Ed25519, remaining 32 bytes = private key.
pub fn load_keypair(keystore_path: &Path, expected_address: &Address) -> anyhow::Result<Ed25519PrivateKey> {
    let json_str = std::fs::read_to_string(keystore_path)
        .map_err(|e| anyhow::anyhow!("Cannot read keystore at {}: {}", keystore_path.display(), e))?;
    let entries: Vec<String> = serde_json::from_str(&json_str)?;

    for entry in &entries {
        let raw = base64ct::Base64::decode_vec(entry)
            .map_err(|_| anyhow::anyhow!("Invalid base64 in keystore entry"))?;
        if raw.len() != 33 {
            continue;
        }
        let flag = raw[0];
        if flag != 0x00 {
            // Only Ed25519 supported for now
            continue;
        }
        let key_bytes: [u8; 32] = raw[1..33]
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid private key length"))?;

        let keypair = Ed25519PrivateKey::new(key_bytes);
        let derived: Address = keypair.public_key().derive_address();

        if derived == *expected_address {
            return Ok(keypair);
        }
    }

    anyhow::bail!(
        "No Ed25519 keypair found in keystore matching address: {:?}",
        expected_address
    )
}
