#![allow(clippy::all)]

pub use bindings::*;

#[allow(clippy::all)]
mod bindings {
    use ethers::contract::abigen;

    abigen!(LSSVMPair, "abi/lssvm_pair.json");
}
