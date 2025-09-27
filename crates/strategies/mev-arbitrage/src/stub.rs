use anyhow::Result;
use async_trait::async_trait;
use artemis_core::types::Strategy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MEVArbitrageEvent;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MEVArbitrageAction;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MEVArbitrageConfig;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MEVArbitrageStats;

#[derive(Debug, Clone, Default)]
pub struct CompleteMEVArbitrageStrategy;

impl CompleteMEVArbitrageStrategy {
    pub fn new(_config: MEVArbitrageConfig) -> Result<Self> {
        Ok(Self)
    }

    pub async fn process_mev_event(
        &mut self,
        _event: MEVArbitrageEvent,
    ) -> Result<Vec<MEVArbitrageAction>> {
        Ok(Vec::new())
    }
}

#[async_trait]
impl Strategy<MEVArbitrageEvent, MEVArbitrageAction> for CompleteMEVArbitrageStrategy {
    async fn sync_state(&mut self) -> Result<()> {
        Ok(())
    }

    async fn process_event(&mut self, event: MEVArbitrageEvent) -> Vec<MEVArbitrageAction> {
        match self.process_mev_event(event).await {
            Ok(actions) => actions,
            Err(_) => Vec::new(),
        }
    }
}
