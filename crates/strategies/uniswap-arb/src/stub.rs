use anyhow::Result;
use async_trait::async_trait;
use artemis_core::types::Strategy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Event;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Action;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArbConfig;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArbStats;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StrategyConfig;

#[derive(Debug, Clone, Default)]
pub struct ConfigLoader;

impl ConfigLoader {
    pub fn load(_path: &str) -> Result<StrategyConfig> {
        Ok(StrategyConfig::default())
    }
}

#[derive(Debug, Clone, Default)]
pub struct V2PoolInfo;

#[derive(Debug, Clone, Default)]
pub struct MevShareUniArb<P> {
    _marker: std::marker::PhantomData<P>,
}

impl<P> MevShareUniArb<P> {
    pub fn new() -> Self {
        Self { _marker: std::marker::PhantomData }
    }
}

#[async_trait]
impl<P> Strategy<Event, Action> for MevShareUniArb<P>
where
    P: Send + Sync + 'static,
{
    async fn sync_state(&mut self) -> Result<()> {
        Ok(())
    }

    async fn process_event(&mut self, _event: Event) -> Vec<Action> {
        Vec::new()
    }
}
