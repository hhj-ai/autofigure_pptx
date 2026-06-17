use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CostTracker {
    max_cost_usd: f64,
    spent_usd: f64,
}

impl CostTracker {
    pub fn new(max_cost_usd: f64) -> Result<Self> {
        if !max_cost_usd.is_finite() || max_cost_usd < 0.0 {
            return Err(anyhow!("max-cost-usd must be a non-negative finite number"));
        }
        Ok(Self {
            max_cost_usd,
            spent_usd: 0.0,
        })
    }

    pub fn reserve(&mut self, label: &str, amount_usd: f64) -> Result<()> {
        if amount_usd < 0.0 || !amount_usd.is_finite() {
            return Err(anyhow!("invalid cost estimate for {label}: {amount_usd}"));
        }
        let next = self.spent_usd + amount_usd;
        if next > self.max_cost_usd {
            return Err(anyhow!(
                "cost cap reached before {label}: estimated ${next:.4} exceeds max ${:.4}",
                self.max_cost_usd
            ));
        }
        self.spent_usd = next;
        Ok(())
    }

    pub fn spent_usd(&self) -> f64 {
        self.spent_usd
    }
}

pub const EST_REASONER_INITIAL_USD: f64 = 0.05;
pub const EST_CODER_USD: f64 = 0.05;
pub const EST_VISION_USD: f64 = 0.05;
pub const EST_REASONER_PATCH_USD: f64 = 0.03;
pub const EST_IMAGE_ASSET_USD: f64 = 0.02;
