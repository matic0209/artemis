#![allow(clippy::all)]

pub use bindings::*;

#[allow(clippy::all)]
mod bindings {
    use ethers::contract::abigen;

    pub use crate::shared_types::*;

    abigen!(
        SudoOpenseaArb,
        "abi/sudo_opensea_arb.json",
        type AdditionalRecipient = crate::shared_types::AdditionalRecipient,
        type BasicOrderParameters = crate::shared_types::BasicOrderParameters,
    );
}
