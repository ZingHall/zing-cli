use std::str::FromStr;
use std::time::Duration;
use sui_crypto::ed25519::Ed25519PrivateKey;
use sui_crypto::SuiSigner;
use sui_rpc::field::FieldMask;
use sui_rpc::field::FieldMaskUtil;
use sui_rpc::proto::sui::rpc::v2::{ExecuteTransactionRequest, GetBalanceRequest};
use sui_sdk_types::{Address, Digest, TypeTag};
use sui_transaction_builder::{Function, ObjectInput, TransactionBuilder};

const USDC_COIN_TYPE: &str =
    "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC";
const MIN_PAYMENT_USDC: u64 = 10_000; // 0.01 USDC in micro-units

/// Send exactly 0.01 USDC to the platform address.
/// Returns the transaction digest as a string.
pub async fn send_payment(
    rpc_url: &str,
    keypair: &Ed25519PrivateKey,
    sender: &Address,
    platform_address: &Address,
) -> anyhow::Result<String> {
    // Pre-check: ensure total balance (addr + coins) has enough USDC
    let (addr_bal, coin_bal) = get_usdc_balance(rpc_url, sender).await?;
    if addr_bal + coin_bal < MIN_PAYMENT_USDC {
        anyhow::bail!("Insufficient USDC balance (need at least 0.01 USDC)");
    }

    // Always consolidate to avoid stale RPC cache on address_balance
    if coin_bal > 0 {
        consolidate_usdc_coins(rpc_url, keypair, sender, coin_bal).await?;
    }

    let mut client = sui_rpc::Client::new(rpc_url)
        .map_err(|e| anyhow::anyhow!("Failed to create Sui RPC client: {e}"))?;

    let usdc_type = TypeTag::from_str(USDC_COIN_TYPE)?;

    // 2. Build PTB — withdraw from address balance and send via balance::send_funds
    let mut tx = TransactionBuilder::new();

    let balance_arg = tx.funds_withdrawal_balance(usdc_type.clone(), MIN_PAYMENT_USDC);
    let recipient_arg = tx.pure(platform_address);
    tx.move_call(
        Function::new(
            Address::TWO,
            sui_sdk_types::Identifier::from_static("balance"),
            sui_sdk_types::Identifier::from_static("send_funds"),
        )
        .with_type_args(vec![usdc_type.clone()]),
        vec![balance_arg, recipient_arg],
    );

    tx.set_sender(*sender);
    tx.set_gas_budget(0);

    // 2. Build, sign, execute
    let transaction = tx
        .build(&mut client)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to build transaction: {e}"))?;

    let signature = keypair
        .sign_transaction(&transaction)
        .map_err(|e| anyhow::anyhow!("Signing failed: {e}"))?;

    let request = ExecuteTransactionRequest::new(transaction.into())
        .with_signatures(vec![signature.into()])
        .with_read_mask(FieldMask::from_paths(vec!["digest"]));

    let response = client
        .execute_transaction_and_wait_for_checkpoint(request, Duration::from_secs(60))
        .await
        .map_err(|e| anyhow::anyhow!("Transaction execution failed: {e}"))?
        .into_inner();

    let digest_str = response
        .transaction
        .as_ref()
        .and_then(|t| t.digest.clone())
        .ok_or_else(|| anyhow::anyhow!("No digest in response"))?;

    Ok(digest_str)
}

/// Query USDC address_balance and coin_balance for the given address.
/// Returns (address_balance, coin_balance) in USDC base units.
pub async fn get_usdc_balance(
    rpc_url: &str,
    address: &Address,
) -> anyhow::Result<(u64, u64)> {
    let mut client = sui_rpc::Client::new(rpc_url)
        .map_err(|e| anyhow::anyhow!("Failed to create Sui RPC client: {e}"))?;

    let mut request = GetBalanceRequest::default();
    request.owner = Some(address.to_string());
    request.coin_type = Some(USDC_COIN_TYPE.to_string());

    let response = client
        .state_client()
        .get_balance(request)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get USDC balance: {e}"))?
        .into_inner();

    match response.balance {
        Some(b) => Ok((
            b.address_balance.unwrap_or(0),
            b.coin_balance.unwrap_or(0),
        )),
        None => Ok((0, 0)),
    }
}

/// Consolidate all owned USDC coins into address balance via coin::send_funds.
/// Executes a single PTB with one move_call per coin.
pub async fn consolidate_usdc_coins(
    rpc_url: &str,
    keypair: &Ed25519PrivateKey,
    sender: &Address,
    coin_bal: u64,
) -> anyhow::Result<String> {
    let mut client = sui_rpc::Client::new(rpc_url)
        .map_err(|e| anyhow::anyhow!("Failed to create Sui RPC client: {e}"))?;

    let usdc_type = TypeTag::from_str(USDC_COIN_TYPE)?;

    let usdc_coins = client
        .select_coins(sender, &usdc_type, coin_bal, &[])
        .await
        .map_err(|e| anyhow::anyhow!("Failed to find USDC coins: {e}"))?;

    if usdc_coins.is_empty() {
        return Ok("no USDC coins to consolidate".into());
    }

    let mut tx = TransactionBuilder::new();

    for usdc_coin in &usdc_coins {
        let obj_id = Address::from_str(usdc_coin.object_id())?;
        let digest = Digest::from_str(usdc_coin.digest())?;
        let coin_arg = tx.object(ObjectInput::owned(obj_id, usdc_coin.version(), digest));
        let self_arg = tx.pure(sender);
        tx.move_call(
            Function::new(
                Address::TWO,
                sui_sdk_types::Identifier::from_static("coin"),
                sui_sdk_types::Identifier::from_static("send_funds"),
            )
            .with_type_args(vec![usdc_type.clone()]),
            vec![coin_arg, self_arg],
        );
    }

    tx.set_sender(*sender);
    tx.set_gas_budget(0);

    let transaction = tx
        .build(&mut client)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to build consolidation transaction: {e}"))?;

    let signature = keypair
        .sign_transaction(&transaction)
        .map_err(|e| anyhow::anyhow!("Signing failed: {e}"))?;

    let request = ExecuteTransactionRequest::new(transaction.into())
        .with_signatures(vec![signature.into()])
        .with_read_mask(FieldMask::from_paths(vec!["digest"]));

    let response = client
        .execute_transaction_and_wait_for_checkpoint(request, Duration::from_secs(60))
        .await
        .map_err(|e| anyhow::anyhow!("Consolidation transaction failed: {e}"))?
        .into_inner();

    let digest_str = response
        .transaction
        .as_ref()
        .and_then(|t| t.digest.clone())
        .ok_or_else(|| anyhow::anyhow!("No digest in consolidation response"))?;

    Ok(digest_str)
}
