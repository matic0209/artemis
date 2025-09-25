#![allow(clippy::all)]

pub use bindings::*;

#[allow(clippy::all)]
mod bindings {
    use ethers::contract::abigen;

    abigen!(SudoPairQuoter, "abi/sudo_pair_quoter.json");

    pub static SUDOPAIRQUOTER_DEPLOYED_BYTECODE: ethers::core::types::Bytes =
        ethers::core::types::Bytes::from_static(include_bytes!(
            "../abi/sudo_pair_quoter_deployed.bin"
        ));
}
