use alloy_primitives::{Address, U256};
use eyre::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{LazyLock, RwLock},
};

use crate::chainspec::{MAINNET_CHAIN_ID, TESTNET_CHAIN_ID};

pub mod init;
mod patch;

static SPOT_META_API_URL: LazyLock<RwLock<Option<String>>> = LazyLock::new(|| RwLock::new(None));

pub fn set_spot_meta_api_url(url: Option<String>) {
    let url = url.and_then(|url| {
        let url = url.trim().to_owned();
        (!url.is_empty()).then_some(url)
    });
    *SPOT_META_API_URL.write().unwrap() = url;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EvmContract {
    address: Address,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpotToken {
    index: u64,
    #[serde(rename = "evmContract")]
    evm_contract: Option<EvmContract>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotMeta {
    tokens: Vec<SpotToken>,
}

#[derive(Debug, Clone)]
pub struct SpotId {
    pub index: u64,
}

impl SpotId {
    pub(crate) fn to_s(&self) -> U256 {
        let mut addr = [0u8; 32];
        addr[12] = 0x20;
        addr[24..32].copy_from_slice(self.index.to_be_bytes().as_ref());
        U256::from_be_bytes(addr)
    }
}

fn fetch_spot_meta(chain_id: u64) -> Result<SpotMeta> {
    let custom_url = SPOT_META_API_URL.read().unwrap().clone();
    let url = match custom_url.as_deref() {
        Some(url) => url,
        None => match chain_id {
            MAINNET_CHAIN_ID => "https://api.hyperliquid.xyz/info",
            TESTNET_CHAIN_ID => "https://api.hyperliquid-testnet.xyz/info",
            _ => return Err(Error::msg("unknown chain id")),
        },
    };
    let response = ureq::post(url)
        .header("Content-Type", "application/json")
        .send(serde_json::json!({"type": "spotMeta"}).to_string())?
        .into_body()
        .read_to_string()?;
    Ok(serde_json::from_str(&response)?)
}

pub(crate) fn erc20_contract_to_spot_token(chain_id: u64) -> Result<BTreeMap<Address, SpotId>> {
    let meta = fetch_spot_meta(chain_id)?;
    let mut map = BTreeMap::new();
    for token in &meta.tokens {
        if let Some(evm_contract) = &token.evm_contract {
            map.insert(evm_contract.address, SpotId { index: token.index });
        }
    }

    if chain_id == TESTNET_CHAIN_ID {
        patch::patch_testnet_spot_meta(&mut map);
    }

    Ok(map)
}
