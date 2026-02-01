pub mod types;

use std::str::FromStr;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use solana_address_lookup_table_interface::state::AddressLookupTable;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    address_lookup_table::AddressLookupTableAccount,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use tracing::debug;

use crate::{
    error::SwapError,
    types::{Provider, QuoteRequest, QuoteResponse, SwapResult},
};

use self::types::{
    JupiterInstruction, JupiterQuoteApiResponse, JupiterQuoteParams,
    JupiterSwapInstructionsResponse, JupiterSwapRequest,
};

const DEFAULT_JUPITER_API_URL: &str = "https://lite-api.jup.ag/swap/v1";

pub struct JupiterProvider {
    pub client: reqwest::Client,
    pub base_url: String,
    pub api_key: Option<String>,
}

impl JupiterProvider {
    pub fn new(base_url: Option<String>, api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.unwrap_or_else(|| DEFAULT_JUPITER_API_URL.to_string()),
            api_key,
        }
    }

    pub async fn quote(
        &self,
        request: &QuoteRequest,
        default_slippage_bps: u16,
    ) -> Result<QuoteResponse, SwapError> {
        let params = JupiterQuoteParams {
            input_mint: request.input_mint.to_string(),
            output_mint: request.output_mint.to_string(),
            amount: request.amount,
            slippage_bps: request.slippage_bps.unwrap_or(default_slippage_bps),
        };

        let url = format!("{}/quote", self.base_url);
        let mut req = self.client.get(&url).query(&params);
        if let Some(key) = &self.api_key {
            req = req.header("x-api-key", key);
        }

        debug!("jupiter quote: {url}");
        let response = req.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if body.contains("No route found") || body.contains("could not find any route") {
                return Err(SwapError::NoRouteFound);
            }
            return Err(SwapError::Api {
                provider: Provider::Jupiter,
                message: format!("HTTP {status}: {body}"),
            });
        }

        let raw_json: serde_json::Value = response.json().await?;
        let api_response: JupiterQuoteApiResponse = serde_json::from_value(raw_json.clone())
            .map_err(|e| SwapError::Serialization(e.to_string()))?;

        let in_amount: u64 = api_response
            .in_amount
            .parse()
            .map_err(|e: std::num::ParseIntError| SwapError::Serialization(e.to_string()))?;
        let out_amount: u64 = api_response
            .out_amount
            .parse()
            .map_err(|e: std::num::ParseIntError| SwapError::Serialization(e.to_string()))?;

        let price_impact_bps = api_response
            .price_impact_pct
            .and_then(|pct| pct.parse::<f64>().ok().map(|p| (p * 100.0) as u16));

        Ok(QuoteResponse {
            provider: Provider::Jupiter,
            input_mint: request.input_mint,
            output_mint: request.output_mint,
            input_amount: in_amount,
            output_amount: out_amount,
            price_impact_bps,
            slippage_bps: api_response.slippage_bps,
            provider_data: raw_json,
        })
    }

    pub async fn swap(
        &self,
        quote: &QuoteResponse,
        user_pubkey: &Pubkey,
        rpc_client: &RpcClient,
    ) -> Result<SwapResult, SwapError> {
        let swap_request = JupiterSwapRequest {
            user_public_key: user_pubkey.to_string(),
            quote_response: quote.provider_data.clone(),
            dynamic_compute_unit_limit: true,
        };

        let url = format!("{}/swap-instructions", self.base_url);
        let mut req = self.client.post(&url).json(&swap_request);
        if let Some(key) = &self.api_key {
            req = req.header("x-api-key", key);
        }

        debug!("jupiter swap-instructions: {url}");
        let response = req.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(SwapError::Api {
                provider: Provider::Jupiter,
                message: format!("HTTP {status}: {body}"),
            });
        }

        let api_response: JupiterSwapInstructionsResponse = response
            .json()
            .await
            .map_err(|e| SwapError::Serialization(e.to_string()))?;

        let mut instructions = Vec::new();

        if let Some(ref ix) = api_response.token_ledger_instruction {
            instructions.push(convert_instruction(ix)?);
        }
        for ix in &api_response.compute_budget_instructions {
            instructions.push(convert_instruction(ix)?);
        }
        for ix in &api_response.setup_instructions {
            instructions.push(convert_instruction(ix)?);
        }
        instructions.push(convert_instruction(&api_response.swap_instruction)?);
        if let Some(ref ix) = api_response.cleanup_instruction {
            instructions.push(convert_instruction(ix)?);
        }
        for ix in &api_response.other_instructions {
            instructions.push(convert_instruction(ix)?);
        }

        let alt_addresses: Vec<Pubkey> = api_response
            .address_lookup_table_addresses
            .iter()
            .filter_map(|s| Pubkey::from_str(s).ok())
            .collect();

        let address_lookup_tables = fetch_address_lookup_tables(&alt_addresses, rpc_client).await?;

        Ok(SwapResult::Instructions {
            instructions,
            address_lookup_tables,
            compute_units: if api_response.compute_unit_limit > 0 {
                Some(api_response.compute_unit_limit)
            } else {
                None
            },
        })
    }
}

fn convert_instruction(ix: &JupiterInstruction) -> Result<Instruction, SwapError> {
    let program_id =
        Pubkey::from_str(&ix.program_id).map_err(|e| SwapError::Serialization(e.to_string()))?;

    let accounts: Vec<AccountMeta> = ix
        .accounts
        .iter()
        .map(|a| {
            let pubkey =
                Pubkey::from_str(&a.pubkey).map_err(|e| SwapError::Serialization(e.to_string()))?;
            Ok(if a.is_writable {
                AccountMeta::new(pubkey, a.is_signer)
            } else {
                AccountMeta::new_readonly(pubkey, a.is_signer)
            })
        })
        .collect::<Result<Vec<_>, SwapError>>()?;

    let data = BASE64
        .decode(&ix.data)
        .map_err(|e| SwapError::Serialization(e.to_string()))?;

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

async fn fetch_address_lookup_tables(
    addresses: &[Pubkey],
    rpc_client: &RpcClient,
) -> Result<Vec<AddressLookupTableAccount>, SwapError> {
    let mut tables = Vec::new();
    for key in addresses {
        let account = rpc_client
            .get_account(key)
            .await
            .map_err(|e| SwapError::Solana(e.to_string()))?;

        let lookup_table = AddressLookupTable::deserialize(&account.data).map_err(
            |e: solana_sdk::instruction::InstructionError| SwapError::Solana(e.to_string()),
        )?;

        tables.push(AddressLookupTableAccount {
            key: *key,
            addresses: lookup_table.addresses.to_vec(),
        });
    }
    Ok(tables)
}
