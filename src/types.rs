use solana_sdk::{
    address_lookup_table::AddressLookupTableAccount,
    hash::Hash,
    instruction::Instruction,
    message::{v0, VersionedMessage},
    pubkey::Pubkey,
    transaction::VersionedTransaction,
};

use crate::error::SwapError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
    Jupiter,
    Titan,
    Dflow,
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Jupiter => write!(f, "Jupiter"),
            Self::Titan => write!(f, "Titan"),
            Self::Dflow => write!(f, "Dflow"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuoteRequest {
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub amount: u64,
    pub slippage_bps: Option<u16>,
}

#[derive(Debug, Clone)]
pub struct QuoteResponse {
    pub provider: Provider,
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub input_amount: u64,
    pub output_amount: u64,
    pub price_impact_bps: Option<u16>,
    pub slippage_bps: u16,
    pub(crate) provider_data: serde_json::Value,
}

#[derive(Debug)]
pub enum SwapResult {
    Instructions {
        instructions: Vec<Instruction>,
        address_lookup_tables: Vec<AddressLookupTableAccount>,
        compute_units: Option<u32>,
    },
    Transaction {
        transaction: VersionedTransaction,
        last_valid_block_height: u64,
    },
}

impl SwapResult {
    pub fn into_unsigned_transaction(
        self,
        payer: &Pubkey,
        blockhash: Hash,
    ) -> Result<VersionedTransaction, SwapError> {
        match self {
            Self::Transaction { transaction, .. } => Ok(transaction),
            Self::Instructions {
                instructions,
                address_lookup_tables,
                ..
            } => {
                let message = v0::Message::try_compile(
                    payer,
                    &instructions,
                    &address_lookup_tables,
                    blockhash,
                )
                .map_err(|e| SwapError::Solana(e.to_string()))?;
                Ok(VersionedTransaction::try_new(
                    VersionedMessage::V0(message),
                    &[] as &[&dyn solana_sdk::signer::Signer],
                )
                .map_err(|e| SwapError::Solana(e.to_string()))?)
            }
        }
    }
}

pub struct SwapConfig {
    pub default_slippage_bps: u16,
    pub jupiter_api_url: Option<String>,
    pub jupiter_api_key: Option<String>,
    pub titan_ws_url: Option<String>,
    pub titan_token: Option<String>,
    pub dflow_api_url: Option<String>,
    pub dflow_api_key: Option<String>,
}
