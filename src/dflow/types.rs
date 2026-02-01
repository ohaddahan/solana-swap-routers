use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DflowOrderResponse {
    pub input_mint: String,
    pub output_mint: String,
    pub in_amount: String,
    pub out_amount: String,
    pub slippage_bps: u16,
    #[serde(default)]
    pub price_impact_pct: Option<String>,
    #[serde(default)]
    pub transaction: Option<String>,
    #[serde(default)]
    pub last_valid_block_height: Option<u64>,
    #[serde(default)]
    pub compute_unit_limit: Option<u32>,
}
