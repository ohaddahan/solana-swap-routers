use serde::Serialize;
use solana_sdk::{
    address_lookup_table::AddressLookupTableAccount,
    hash::Hash,
    instruction::{AccountMeta, Instruction},
    message::{v0, VersionedMessage},
    pubkey,
    pubkey::Pubkey,
    signature::Signature,
    transaction::VersionedTransaction,
};

use crate::error::SwapError;

pub const JUPITER_PROGRAM: Pubkey = pubkey!("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4");
pub const TITAN_PROGRAM: Pubkey = pubkey!("T1TANpTeScyeqVzzgNViGDNrkQ6qHz9KrSBS4aNXvGT");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SwapMode {
    ExactIn,
    ExactOut,
}

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
    pub only_direct_routes: Option<bool>,
    pub taker: Option<Pubkey>,
    pub restrict_intermediate_tokens: Option<bool>,
    pub as_legacy_transaction: Option<bool>,
    pub swap_mode: Option<SwapMode>,
    pub dexes: Option<String>,
    pub exclude_dexes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuoteResponse {
    pub provider: Provider,
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub input_amount: u64,
    pub output_amount: u64,
    pub price_impact_bps: Option<u16>,
    pub slippage_bps: u16,
    pub provider_data: serde_json::Value,
}

impl Serialize for Provider {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
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

#[derive(Debug)]
pub struct CpiSwapResult {
    pub swap_data: Vec<u8>,
    pub remaining_accounts: Vec<AccountMeta>,
    pub pre_instructions: Vec<Instruction>,
    pub post_instructions: Vec<Instruction>,
    pub address_lookup_tables: Vec<AddressLookupTableAccount>,
}

impl SwapResult {
    pub fn into_cpi(self, executor_program: Pubkey) -> Result<CpiSwapResult, SwapError> {
        match self {
            Self::Instructions {
                instructions,
                address_lookup_tables,
                ..
            } => {
                let swap_idx = instructions
                    .iter()
                    .position(|ix| ix.program_id == executor_program)
                    .ok_or_else(|| {
                        SwapError::Solana(format!(
                            "no instruction found for executor program {executor_program}"
                        ))
                    })?;

                let swap_ix = &instructions[swap_idx];
                let swap_data = swap_ix.data.clone();
                let remaining_accounts = swap_ix.accounts.clone();
                let pre_instructions = instructions[..swap_idx].to_vec();
                let post_instructions = instructions[swap_idx + 1..].to_vec();

                Ok(CpiSwapResult {
                    swap_data,
                    remaining_accounts,
                    pre_instructions,
                    post_instructions,
                    address_lookup_tables,
                })
            }
            Self::Transaction { .. } => Err(SwapError::Solana(
                "into_cpi is only supported for Instructions variant".to_string(),
            )),
        }
    }

    pub fn into_unsigned_transaction(
        self,
        payer: &Pubkey,
        blockhash: Hash,
    ) -> Result<VersionedTransaction, SwapError> {
        match self {
            Self::Transaction {
                mut transaction, ..
            } => {
                match &mut transaction.message {
                    VersionedMessage::Legacy(m) => m.recent_blockhash = blockhash,
                    VersionedMessage::V0(m) => m.recent_blockhash = blockhash,
                }
                let num_signers = transaction.message.header().num_required_signatures as usize;
                transaction.signatures = vec![Signature::default(); num_signers];
                Ok(transaction)
            }
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
                let num_signers = message.header.num_required_signatures as usize;
                let message = VersionedMessage::V0(message);
                Ok(VersionedTransaction {
                    signatures: vec![Signature::default(); num_signers],
                    message,
                })
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
    pub dflow_max_route_length: Option<u32>,
}

#[cfg(test)]
#[expect(clippy::expect_used, reason = "test code")]
mod tests {
    use super::*;

    fn make_instruction(program_id: Pubkey, data: &[u8]) -> Instruction {
        Instruction {
            program_id,
            accounts: vec![AccountMeta::new(Pubkey::new_unique(), true)],
            data: data.to_vec(),
        }
    }

    fn make_swap_result(instructions: Vec<Instruction>) -> SwapResult {
        SwapResult::Instructions {
            instructions,
            address_lookup_tables: vec![],
            compute_units: Some(200_000),
        }
    }

    #[test]
    fn into_cpi_splits_pre_swap_post() {
        let pre_program = Pubkey::new_unique();
        let executor = Pubkey::new_unique();
        let post_program = Pubkey::new_unique();

        let result = make_swap_result(vec![
            make_instruction(pre_program, &[1, 2]),
            make_instruction(executor, &[3, 4, 5]),
            make_instruction(post_program, &[6]),
        ]);

        let cpi = result.into_cpi(executor).expect("into_cpi should succeed");

        assert_eq!(cpi.swap_data, vec![3, 4, 5]);
        assert_eq!(cpi.pre_instructions.len(), 1);
        assert_eq!(cpi.pre_instructions[0].program_id, pre_program);
        assert_eq!(cpi.post_instructions.len(), 1);
        assert_eq!(cpi.post_instructions[0].program_id, post_program);
    }

    #[test]
    fn into_cpi_executor_is_first_instruction() {
        let executor = Pubkey::new_unique();
        let post_program = Pubkey::new_unique();

        let result = make_swap_result(vec![
            make_instruction(executor, &[10]),
            make_instruction(post_program, &[20]),
        ]);

        let cpi = result.into_cpi(executor).expect("into_cpi should succeed");

        assert_eq!(cpi.swap_data, vec![10]);
        assert!(cpi.pre_instructions.is_empty());
        assert_eq!(cpi.post_instructions.len(), 1);
    }

    #[test]
    fn into_cpi_executor_is_last_instruction() {
        let pre_program = Pubkey::new_unique();
        let executor = Pubkey::new_unique();

        let result = make_swap_result(vec![
            make_instruction(pre_program, &[1]),
            make_instruction(executor, &[99]),
        ]);

        let cpi = result.into_cpi(executor).expect("into_cpi should succeed");

        assert_eq!(cpi.swap_data, vec![99]);
        assert_eq!(cpi.pre_instructions.len(), 1);
        assert!(cpi.post_instructions.is_empty());
    }

    #[test]
    fn into_cpi_single_instruction() {
        let executor = Pubkey::new_unique();
        let result = make_swap_result(vec![make_instruction(executor, &[42])]);

        let cpi = result.into_cpi(executor).expect("into_cpi should succeed");

        assert_eq!(cpi.swap_data, vec![42]);
        assert!(cpi.pre_instructions.is_empty());
        assert!(cpi.post_instructions.is_empty());
    }

    #[test]
    fn into_cpi_preserves_accounts() {
        let executor = Pubkey::new_unique();
        let account_key = Pubkey::new_unique();

        let ix = Instruction {
            program_id: executor,
            accounts: vec![
                AccountMeta::new(account_key, false),
                AccountMeta::new_readonly(Pubkey::new_unique(), true),
            ],
            data: vec![1],
        };
        let result = make_swap_result(vec![ix]);

        let cpi = result.into_cpi(executor).expect("into_cpi should succeed");

        assert_eq!(cpi.remaining_accounts.len(), 2);
        assert_eq!(cpi.remaining_accounts[0].pubkey, account_key);
        assert!(!cpi.remaining_accounts[0].is_signer);
        assert!(cpi.remaining_accounts[0].is_writable);
    }

    #[test]
    fn into_cpi_preserves_address_lookup_tables() {
        let executor = Pubkey::new_unique();
        let alt_key = Pubkey::new_unique();
        let alt_address = Pubkey::new_unique();

        let result = SwapResult::Instructions {
            instructions: vec![make_instruction(executor, &[1])],
            address_lookup_tables: vec![AddressLookupTableAccount {
                key: alt_key,
                addresses: vec![alt_address],
            }],
            compute_units: None,
        };

        let cpi = result.into_cpi(executor).expect("into_cpi should succeed");

        assert_eq!(cpi.address_lookup_tables.len(), 1);
        assert_eq!(cpi.address_lookup_tables[0].key, alt_key);
        assert_eq!(cpi.address_lookup_tables[0].addresses, vec![alt_address]);
    }

    #[test]
    fn into_cpi_no_matching_executor_returns_error() {
        let other_program = Pubkey::new_unique();
        let executor = Pubkey::new_unique();
        let result = make_swap_result(vec![make_instruction(other_program, &[1])]);

        let err = result.into_cpi(executor).expect_err("should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("no instruction found"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn into_cpi_transaction_variant_returns_error() {
        let payer = Pubkey::new_unique();
        let ix = make_instruction(payer, &[1]);

        let msg = v0::Message::try_compile(&payer, &[ix], &[], Hash::default())
            .expect("should compile");
        let versioned_msg = VersionedMessage::V0(msg);
        let tx = VersionedTransaction {
            signatures: vec![Signature::default()],
            message: versioned_msg,
        };
        let result = SwapResult::Transaction {
            transaction: tx,
            last_valid_block_height: 100,
        };

        let err = result.into_cpi(payer).expect_err("should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("only supported for Instructions"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn into_cpi_multiple_pre_and_post() {
        let pre1 = Pubkey::new_unique();
        let pre2 = Pubkey::new_unique();
        let executor = Pubkey::new_unique();
        let post1 = Pubkey::new_unique();
        let post2 = Pubkey::new_unique();
        let post3 = Pubkey::new_unique();

        let result = make_swap_result(vec![
            make_instruction(pre1, &[1]),
            make_instruction(pre2, &[2]),
            make_instruction(executor, &[3]),
            make_instruction(post1, &[4]),
            make_instruction(post2, &[5]),
            make_instruction(post3, &[6]),
        ]);

        let cpi = result.into_cpi(executor).expect("into_cpi should succeed");

        assert_eq!(cpi.pre_instructions.len(), 2);
        assert_eq!(cpi.pre_instructions[0].program_id, pre1);
        assert_eq!(cpi.pre_instructions[1].program_id, pre2);
        assert_eq!(cpi.post_instructions.len(), 3);
        assert_eq!(cpi.post_instructions[0].program_id, post1);
        assert_eq!(cpi.post_instructions[1].program_id, post2);
        assert_eq!(cpi.post_instructions[2].program_id, post3);
    }

    #[test]
    fn into_cpi_uses_first_match_when_duplicated() {
        let executor = Pubkey::new_unique();

        let result = make_swap_result(vec![
            make_instruction(executor, &[1]),
            make_instruction(executor, &[2]),
        ]);

        let cpi = result.into_cpi(executor).expect("into_cpi should succeed");

        assert_eq!(cpi.swap_data, vec![1], "should match first occurrence");
        assert_eq!(cpi.post_instructions.len(), 1);
        assert_eq!(cpi.post_instructions[0].data, vec![2]);
    }

    #[test]
    fn quote_response_serializes_to_json() {
        let quote = QuoteResponse {
            provider: Provider::Jupiter,
            input_mint: Pubkey::new_unique(),
            output_mint: Pubkey::new_unique(),
            input_amount: 1_000_000,
            output_amount: 500_000,
            price_impact_bps: Some(15),
            slippage_bps: 100,
            provider_data: serde_json::json!({"route_plan": []}),
        };

        let json = serde_json::to_value(&quote).expect("should serialize");

        assert_eq!(json["provider"], "Jupiter");
        assert_eq!(json["input_amount"], 1_000_000);
        assert_eq!(json["output_amount"], 500_000);
        assert_eq!(json["price_impact_bps"], 15);
        assert_eq!(json["slippage_bps"], 100);
    }

    #[test]
    fn provider_display_and_serialize_match() {
        for (provider, expected) in [
            (Provider::Jupiter, "Jupiter"),
            (Provider::Titan, "Titan"),
            (Provider::Dflow, "Dflow"),
        ] {
            assert_eq!(provider.to_string(), expected);
            let json = serde_json::to_value(provider).expect("should serialize");
            assert_eq!(json, expected);
        }
    }

    #[test]
    fn swap_mode_equality() {
        assert_eq!(SwapMode::ExactIn, SwapMode::ExactIn);
        assert_ne!(SwapMode::ExactIn, SwapMode::ExactOut);
    }
}
