#![cfg(feature = "sdk-alloy")]

use artemis_core::eth::LocalWallet;

fn main() {
    let wallet = LocalWallet::random();
    let private_key = wallet.to_bytes();
    let address = wallet.address();

    println!("Private key: 0x{}", hex::encode(private_key.as_slice()));
    println!("Address: 0x{}", hex::encode(address.as_slice()));
}
