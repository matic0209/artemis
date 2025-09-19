set dotenv-load := true

#test contracts using forge
build-contracts:
    forge install --root ./contracts
    forge test --root ./contracts


#test contracts using forge
test-contracts: 
    forge test --root ./contracts

#download source code from etherscan for members of the protocols.json file
download-protocol-sources: 
    #!/usr/bin/env bash
    for name in $(jq -r 'keys[]' protocols.json); do
        address=$(jq -r ".$name" protocols.json)
        echo "Downloading $name from $address"
        cast etherscan-source --etherscan-api-key $ETHERSCAN_API_KEY -d ./contracts/src/protocols $address
    done

#generate bindings for elements of the contracts directory
generate-bindings: 
    forge bind --bindings-path ./bindings --root ./contracts --crate-name bindings --force

#download sources and generate bindings
build-bindings-crate: download-protocol-sources generate-bindings

fmt: 
    cargo +nightly fmt --all

clippy: 
    cargo clippy --all --all-features

perf-smoke:
    cargo test --package artemis-core -- --nocapture --test-threads=1

bench:
    if ! cargo bench --workspace; then \
        echo "warning: cargo bench failed (no benchmarks configured yet)"; \
    fi

soak:
    cargo test --workspace --release -- --ignored --test-threads=1
