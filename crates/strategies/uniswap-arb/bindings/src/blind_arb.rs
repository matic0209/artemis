#[allow(clippy::all)]
pub use bindings::*;

#[allow(clippy::all)]
mod bindings {
    use ethers::contract::abigen;

    abigen!(BlindArb, "abi/blind_arb.json");
}
