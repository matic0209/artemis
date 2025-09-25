#![allow(clippy::all)]

pub use bindings::*;

#[allow(clippy::all)]
mod bindings {
    use ethers::contract::abigen;

    abigen!(LSSVMPairFactory, "abi/lssvm_pair_factory.json");
}
