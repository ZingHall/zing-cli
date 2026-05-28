use crate::models::*;
use base64ct::{Base64, Encoding};
use sui_crypto::ed25519::Ed25519PrivateKey;
use sui_crypto::SuiSigner;
use sui_sdk_types::{Address, PersonalMessage};

/// Sign the BCS-encoded ApiAccessMessage as a PersonalMessage.
/// Returns (signature_base64, bytes_base64).
fn sign_access_message(
    keypair: &Ed25519PrivateKey,
    q: &str,
    wiki: &str,
    transaction_digest: &str,
) -> anyhow::Result<(String, String)> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let msg = ApiAccessMessage {
        q: q.to_string(),
        wiki: wiki.to_string(),
        transaction_digest: transaction_digest.to_string(),
        timestamp,
    };

    let bcs_bytes = bcs::to_bytes(&msg)?;
    let bytes_b64 = Base64::encode_string(&bcs_bytes);

    let signature = keypair
        .sign_personal_message(&PersonalMessage(bcs_bytes.clone().into()))
        .map_err(|e| anyhow::anyhow!("Signing ApiAccessMessage failed: {e}"))?;

    let sig_b64 = signature.to_base64();

    Ok((sig_b64, bytes_b64))
}

/// POST to the search endpoint. Returns the response.
#[allow(clippy::too_many_arguments)]
pub async fn search(
    rpc_url: &str,
    api_base_url: &str,
    keypair: &Ed25519PrivateKey,
    sender: &Address,
    platform_usdc_address: &Address,
    q: &str,
    wiki: &str,
    owner: Option<&str>,
    limit: u32,
) -> anyhow::Result<SearchResponse> {
    let tx_digest = crate::sui::send_payment(rpc_url, keypair, sender, platform_usdc_address).await?;

    let (signature, bytes) = sign_access_message(keypair, q, wiki, &tx_digest)?;

    let body = PaidRequest {
        q: q.to_string(),
        wiki: wiki.to_string(),
        owner: owner.map(|s| s.to_string()),
        limit,
        transaction_digest: tx_digest,
        signature,
        bytes,
    };

    let client = reqwest::Client::new();
    let url = format!("{}/search", api_base_url.trim_end_matches('/'));
    let resp = client.post(&url).json(&body).send().await?;

    let status = resp.status();
    if !status.is_success() {
        let body_text = resp.text().await.unwrap_or_default();
        anyhow::bail!("API error ({}): {}", status.as_u16(), body_text);
    }

    let search_resp: SearchResponse = resp.json().await?;
    Ok(search_resp)
}

/// POST to the chunks endpoint. Returns the response.
#[allow(clippy::too_many_arguments)]
pub async fn chunks(
    rpc_url: &str,
    api_base_url: &str,
    keypair: &Ed25519PrivateKey,
    sender: &Address,
    platform_usdc_address: &Address,
    q: &str,
    wiki: &str,
    owner: Option<&str>,
    limit: u32,
) -> anyhow::Result<ChunksResponse> {
    let tx_digest = crate::sui::send_payment(rpc_url, keypair, sender, platform_usdc_address).await?;

    let (signature, bytes) = sign_access_message(keypair, q, wiki, &tx_digest)?;

    let body = PaidRequest {
        q: q.to_string(),
        wiki: wiki.to_string(),
        owner: owner.map(|s| s.to_string()),
        limit,
        transaction_digest: tx_digest,
        signature,
        bytes,
    };

    let client = reqwest::Client::new();
    let url = format!("{}/chunks", api_base_url.trim_end_matches('/'));
    let resp = client.post(&url).json(&body).send().await?;

    let status = resp.status();
    if !status.is_success() {
        let body_text = resp.text().await.unwrap_or_default();
        anyhow::bail!("API error ({}): {}", status.as_u16(), body_text);
    }

    let chunks_resp: ChunksResponse = resp.json().await?;
    Ok(chunks_resp)
}
