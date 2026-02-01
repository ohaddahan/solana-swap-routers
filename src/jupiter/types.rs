use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JupiterQuoteParams {
    pub input_mint: String,
    pub output_mint: String,
    pub amount: u64,
    pub slippage_bps: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub only_direct_routes: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JupiterQuoteApiResponse {
    pub input_mint: String,
    pub output_mint: String,
    pub in_amount: String,
    pub out_amount: String,
    pub slippage_bps: u16,
    #[serde(default)]
    pub other_amount_threshold: Option<String>,
    #[serde(default)]
    pub price_impact_pct: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JupiterSwapRequest {
    pub user_public_key: String,
    pub quote_response: serde_json::Value,
    pub dynamic_compute_unit_limit: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JupiterSwapInstructionsResponse {
    pub token_ledger_instruction: Option<JupiterInstruction>,
    #[serde(default)]
    pub compute_budget_instructions: Vec<JupiterInstruction>,
    #[serde(default)]
    pub setup_instructions: Vec<JupiterInstruction>,
    pub swap_instruction: JupiterInstruction,
    pub cleanup_instruction: Option<JupiterInstruction>,
    #[serde(default)]
    pub other_instructions: Vec<JupiterInstruction>,
    #[serde(default)]
    pub address_lookup_table_addresses: Vec<String>,
    #[serde(default)]
    pub compute_unit_limit: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JupiterInstruction {
    pub program_id: String,
    pub accounts: Vec<JupiterAccountMeta>,
    pub data: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JupiterAccountMeta {
    pub pubkey: String,
    pub is_signer: bool,
    pub is_writable: bool,
}
