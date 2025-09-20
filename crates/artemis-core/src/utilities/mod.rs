//! Utilities for working with Artemis.

/// Alloy-based state override helper. Provides state override functionality
/// for strategies to perform `eth_call` operations with temporary bytecode overrides.
pub mod state_override_middleware {
    use alloy_provider::Provider as ProviderTrait;
    use alloy_rpc_types_eth::state::StateOverride;
    use anyhow::{anyhow, Result};

    use crate::eth::{Address, BlockId, Bytes, Provider, TxRequest, U256};

    #[derive(Debug, Clone)]
    pub struct StateOverrideMiddleware {
        provider: Provider,
        overrides: StateOverride,
        next_scratch: u64,
    }

    impl StateOverrideMiddleware {
        pub fn new(provider: Provider) -> Self {
            Self {
                provider,
                overrides: StateOverride::default(),
                next_scratch: 1,
            }
        }

        pub fn add_code_to_address(&mut self, address: Address, code: Bytes) {
            self.overrides.entry(address).or_default().set_code(code);
        }

        pub fn set_balance(&mut self, address: Address, balance: U256) {
            self.overrides
                .entry(address)
                .or_default()
                .set_balance(balance);
        }

        pub fn add_code(&mut self, code: Bytes) -> Address {
            let mut raw = [0u8; 20];
            raw[12..].copy_from_slice(&self.next_scratch.to_be_bytes());
            self.next_scratch += 1;
            let addr = Address::from(raw);
            self.add_code_to_address(addr, code);
            addr
        }

        pub fn provider(&self) -> &Provider {
            &self.provider
        }

        pub async fn call_with_overrides(
            &self,
            tx: TxRequest,
            block: Option<BlockId>,
        ) -> Result<Bytes> {
            let mut call =
                ProviderTrait::call(&self.provider, tx).overrides(self.overrides.clone());
            if let Some(block) = block {
                call = call.block(block);
            }

            call.await
                .map_err(|err| anyhow!("eth_call with overrides failed: {err}"))
        }
    }
}
