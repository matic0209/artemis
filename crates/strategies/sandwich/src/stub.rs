use anyhow::Result;
use async_trait::async_trait;
use artemis_core::types::Strategy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Event {
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Action {
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SandwichConfig;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SandwichBundle;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SandwichOpportunity;

#[derive(Debug, Clone, Default)]
pub struct SandwichStrategy<P> {
    _marker: std::marker::PhantomData<P>,
}

impl<P> SandwichStrategy<P> {
    pub fn new() -> Self {
        Self { _marker: std::marker::PhantomData }
    }
}

#[async_trait]
impl<P> Strategy<Event, Action> for SandwichStrategy<P>
where
    P: Send + Sync + 'static,
{
    async fn sync_state(&mut self) -> Result<()> {
        Ok(())
    }

    async fn process_event(&mut self, event: Event) -> Vec<Action> {
        vec![Action { details: format!("stub action for {}", event.description) }]
    }
}

#[derive(Debug, Clone, Default)]
pub struct SandwichMempoolCollector;
#[derive(Debug, Clone, Default)]
pub struct SandwichExecutor;
#[derive(Debug, Clone, Default)]
pub struct StateQuerier;
#[derive(Debug, Clone, Default)]
pub struct PoolStateMonitor;
#[derive(Debug, Clone, Default)]
pub struct RevmEngine;
#[derive(Debug, Clone, Default)]
pub struct RevmConfig;
#[derive(Debug, Clone, Default)]
pub struct TransactionExecutor;
#[derive(Debug, Clone, Default)]
pub struct SandwichSimulationResult;
