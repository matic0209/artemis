use std::sync::Arc;

use anyhow::Result;
use artemis_core::collectors::mevshare_collector::MevShareCollector;
use artemis_core::engine::Engine;
use artemis_core::eth::Address;
use artemis_core::types::{CollectorMap, ExecutorMap};
use clap::Parser;
use mev_share_uni_arb::{
    strategy::MevShareUniArb,
    types::{Action, Event},
};
use tracing::{info, Level};
use tracing_subscriber::{filter, prelude::*};

// MEV share example using Alloy

#[cfg(feature = "sdk-ethers")]
use artemis_core::eth::{
    helpers as ethers_helpers, LocalWallet as EthersWallet, MiddlewareBuilder, Signer,
};
#[cfg(feature = "sdk-ethers")]
use artemis_core::executors::mev_share_executor::MevshareExecutor;

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use alloy_primitives::{Address as AlloyAddress, Bytes as AlloyBytes, B256 as AlloyB256};
#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use alloy_rpc_types_mev::{
    BundleItem as AlloyBundleItem, Inclusion as AlloyInclusion, MevSendBundle,
    Privacy as AlloyPrivacy, PrivacyHint as AlloyPrivacyHint,
    ProtocolVersion as AlloyProtocolVersion, Refund as AlloyRefund,
    RefundConfig as AlloyRefundConfig, Validity as AlloyValidity,
};
#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use artemis_core::eth::alloy_support::{helpers as alloy_helpers, LocalWallet as AlloyWallet};
#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use artemis_core::executors::mev_share_alloy_executor::MevshareAlloyExecutor;
#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use mev_share::rpc::{
    BundleItem as MevBundleItem, Inclusion as MevInclusion, Privacy as MevPrivacy,
    PrivacyHint as MevPrivacyHint, ProtocolVersion as MevProtocolVersion, Refund as MevRefund,
    RefundConfig as MevRefundConfig, SendBundleRequest, Validity as MevValidity,
};

/// CLI Options.
#[derive(Parser, Debug)]
pub struct Args {
    /// Ethereum node WS endpoint.
    #[arg(long)]
    pub wss: String,
    /// Private key for sending txs.
    #[arg(long)]
    pub private_key: String,
    /// MEV share signer
    #[arg(long)]
    pub flashbots_signer: String,
    /// Address of the arb contract.
    #[arg(long)]
    pub arb_contract_address: Address,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Set up tracing and parse args.
    let filter = filter::Targets::new()
        .with_target("mev_share_uni_arb", Level::INFO)
        .with_target("artemis_core", Level::INFO);
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .init();

    let args = Args::parse();
    run_app(args).await
}

#[cfg(feature = "sdk-ethers")]
async fn run_app(args: Args) -> Result<()> {
    let provider = ethers_helpers::create_ws_provider(&args.wss).await?;
    let wallet: EthersWallet = ethers_helpers::parse_local_wallet(&args.private_key)?;
    let address = wallet.address();
    let provider = Arc::new(provider.nonce_manager(address).with_signer(wallet.clone()));
    let fb_signer: EthersWallet = args.flashbots_signer.parse()?;

    let mut engine: Engine<Event, Action> = Engine::default();

    let mevshare_collector = Box::new(MevShareCollector::new(String::from(
        "https://mev-share.flashbots.net",
    )));
    let mevshare_collector = CollectorMap::new(mevshare_collector, Event::MEVShareEvent);
    engine.add_collector(Box::new(mevshare_collector));

    let strategy = MevShareUniArb::new(Arc::clone(&provider), wallet, args.arb_contract_address);
    engine.add_strategy(Box::new(strategy));

    let executor = Box::new(MevshareExecutor::new(fb_signer));
    let executor = ExecutorMap::new(executor, |action| match action {
        Action::SubmitBundle(bundle) => Some(bundle),
    });
    engine.add_executor(Box::new(executor));

    if let Ok(mut set) = engine.run().await {
        while let Some(res) = set.join_next().await {
            info!("res: {:?}", res);
        }
    }

    Ok(())
}

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
async fn run_app(args: Args) -> Result<()> {
    let provider = alloy_helpers::create_ws_provider(&args.wss).await?;
    let provider = Arc::new(provider);
    let wallet: AlloyWallet = alloy_helpers::parse_local_wallet(&args.private_key)?;
    let fb_signer: AlloyWallet = alloy_helpers::parse_local_wallet(&args.flashbots_signer)?;

    let mut engine: Engine<Event, Action> = Engine::default();

    let mevshare_collector = Box::new(MevShareCollector::new(String::from(
        "https://mev-share.flashbots.net",
    )));
    let mevshare_collector = CollectorMap::new(mevshare_collector, Event::MEVShareEvent);
    engine.add_collector(Box::new(mevshare_collector));

    let strategy = MevShareUniArb::new(Arc::clone(&provider), wallet, args.arb_contract_address);
    engine.add_strategy(Box::new(strategy));

    let executor = Box::new(MevshareAlloyExecutor::new(Arc::clone(&provider), fb_signer));
    let executor = ExecutorMap::new(executor, |action| match action {
        Action::SubmitBundle(bundle) => convert_bundle(bundle),
    });
    engine.add_executor(Box::new(executor));

    if let Ok(mut set) = engine.run().await {
        while let Some(res) = set.join_next().await {
            info!("res: {:?}", res);
        }
    }

    Ok(())
}

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
fn convert_bundle(bundle: SendBundleRequest) -> Option<MevSendBundle> {
    let inclusion = convert_inclusion(bundle.inclusion);
    let body = bundle
        .bundle_body
        .into_iter()
        .map(convert_bundle_item)
        .collect::<Option<Vec<_>>>()?;
    let validity = bundle.validity.map(convert_validity);
    let privacy = bundle.privacy.map(convert_privacy);

    Some(MevSendBundle {
        protocol_version: convert_protocol_version(bundle.protocol_version),
        inclusion,
        bundle_body: body,
        validity,
        privacy,
    })
}

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
fn convert_protocol_version(version: MevProtocolVersion) -> AlloyProtocolVersion {
    match version {
        MevProtocolVersion::Beta1 => AlloyProtocolVersion::Beta1,
        MevProtocolVersion::V0_1 => AlloyProtocolVersion::V0_1,
    }
}

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
fn convert_inclusion(inclusion: MevInclusion) -> AlloyInclusion {
    let block = inclusion.block.as_u64();
    let max_block = inclusion.max_block.map(|val| val.as_u64());

    AlloyInclusion { block, max_block }
}

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
fn convert_bundle_item(item: MevBundleItem) -> Option<AlloyBundleItem> {
    match item {
        MevBundleItem::Hash { hash } => {
            let alloy_hash = AlloyB256::from_slice(hash.as_bytes());
            Some(AlloyBundleItem::Hash { hash: alloy_hash })
        }
        MevBundleItem::Tx { tx, can_revert } => {
            let alloy_bytes = AlloyBytes::from(tx.to_vec());
            Some(AlloyBundleItem::Tx {
                tx: alloy_bytes,
                can_revert,
            })
        }
    }
}

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
fn convert_validity(validity: MevValidity) -> AlloyValidity {
    let refund = validity
        .refund
        .map(|items| items.into_iter().map(convert_refund).collect::<Vec<_>>());
    let refund_config = validity.refund_config.map(|items| {
        items
            .into_iter()
            .map(convert_refund_config)
            .collect::<Vec<_>>()
    });

    AlloyValidity {
        refund,
        refund_config,
    }
}

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
fn convert_refund(refund: MevRefund) -> AlloyRefund {
    AlloyRefund {
        body_idx: refund.body_idx,
        percent: refund.percent,
    }
}

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
fn convert_refund_config(config: MevRefundConfig) -> AlloyRefundConfig {
    let address = AlloyAddress::from_slice(config.address.as_bytes());
    AlloyRefundConfig {
        address,
        percent: config.percent,
    }
}

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
fn convert_privacy(privacy: MevPrivacy) -> AlloyPrivacy {
    let hints = privacy.hints.map(convert_privacy_hint);
    let builders = privacy.builders.map(|addresses| {
        addresses
            .into_iter()
            .map(|addr| format!("{addr:#x}"))
            .collect::<Vec<_>>()
    });

    AlloyPrivacy { hints, builders }
}

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
fn convert_privacy_hint(hint: MevPrivacyHint) -> AlloyPrivacyHint {
    AlloyPrivacyHint {
        calldata: hint.calldata,
        contract_address: hint.contract_address,
        logs: hint.logs,
        function_selector: hint.function_selector,
        hash: hint.hash,
        tx_hash: hint.tx_hash,
    }
}

#[cfg(all(test, feature = "sdk-alloy", not(feature = "sdk-ethers")))]
mod tests {
    use super::*;
    use alloy_primitives::B256;
    use ethers::types::{Address as EthersAddress, Bytes as EthersBytes, TxHash, U64};

    fn sample_request() -> SendBundleRequest {
        let hash = TxHash::from_slice(&[0x11; 32]);
        let tx_bytes = EthersBytes::from_static(&[0xaa, 0xbb, 0xcc]);
        let refund_address = EthersAddress::from_slice(&[0x22; 20]);
        let builder_address = EthersAddress::from_slice(&[0x33; 20]);

        SendBundleRequest {
            protocol_version: MevProtocolVersion::V0_1,
            inclusion: MevInclusion {
                block: U64::from(1_u64),
                max_block: Some(U64::from(2_u64)),
            },
            bundle_body: vec![
                MevBundleItem::Hash { hash },
                MevBundleItem::Tx {
                    tx: tx_bytes.clone(),
                    can_revert: true,
                },
            ],
            validity: Some(MevValidity {
                refund: Some(vec![MevRefund {
                    body_idx: 1,
                    percent: 10,
                }]),
                refund_config: Some(vec![MevRefundConfig {
                    address: refund_address,
                    percent: 5,
                }]),
            }),
            privacy: Some(MevPrivacy {
                hints: Some(MevPrivacyHint {
                    calldata: true,
                    contract_address: false,
                    logs: true,
                    function_selector: false,
                    hash: true,
                    tx_hash: false,
                }),
                builders: Some(vec![builder_address]),
            }),
        }
    }

    #[test]
    fn converts_mev_share_bundle_to_alloy_bundle() {
        let request = sample_request();
        let converted = convert_bundle(request)
            .expect("conversion should succeed");

        assert_eq!(converted.protocol_version, AlloyProtocolVersion::V0_1);
        assert_eq!(converted.inclusion.block, 1);
        assert_eq!(converted.inclusion.max_block, Some(2));
        assert_eq!(converted.bundle_body.len(), 2);

        match &converted.bundle_body[0] {
            AlloyBundleItem::Hash { hash } => {
                assert_eq!(*hash, B256::from_slice(&[0x11; 32]));
            }
            other => panic!("unexpected bundle item: {:?}", other),
        }

        match &converted.bundle_body[1] {
            AlloyBundleItem::Tx { tx, can_revert } => {
                assert_eq!(tx.as_ref(), &[0xaa, 0xbb, 0xcc]);
                assert!(*can_revert);
            }
            other => panic!("unexpected bundle item: {:?}", other),
        }

        // Test validity section with proper error handling
        match &converted.validity {
            Some(validity) => {
                match &validity.refund {
                    Some(refund) => {
                        assert_eq!(refund.len(), 1);
                        assert_eq!(refund[0].body_idx, 1);
                        assert_eq!(refund[0].percent, 10);
                    }
                    None => panic!("refund expected in validity section"),
                }

                match &validity.refund_config {
                    Some(refund_config) => {
                        assert_eq!(refund_config.len(), 1);
                        assert_eq!(
                            refund_config[0].address,
                            AlloyAddress::from_slice(&[0x22; 20])
                        );
                        assert_eq!(refund_config[0].percent, 5);
                    }
                    None => panic!("refund_config expected in validity section"),
                }
            }
            None => panic!("validity section expected"),
        }

        // Test privacy section with proper error handling
        match &converted.privacy {
            Some(privacy) => {
                match &privacy.hints {
                    Some(hints) => {
                        assert!(hints.calldata);
                        assert!(hints.logs);
                        assert!(hints.hash);
                        assert!(!hints.contract_address);
                        assert!(!hints.function_selector);
                        assert!(!hints.tx_hash);
                    }
                    None => panic!("hints expected in privacy section"),
                }

                match &privacy.builders {
                    Some(builders) => {
                        assert_eq!(builders.len(), 1);
                        assert_eq!(
                            builders[0],
                            format!("{:#x}", EthersAddress::from_slice(&[0x33; 20]))
                        );
                    }
                    None => panic!("builders expected in privacy section"),
                }
            }
            None => panic!("privacy section expected"),
        }
    }

    #[test]
    fn convert_bundle_handles_empty_optional_fields() {
        let request = SendBundleRequest {
            protocol_version: MevProtocolVersion::Beta1,
            inclusion: MevInclusion {
                block: U64::from(5_u64),
                max_block: None,
            },
            bundle_body: vec![MevBundleItem::Tx {
                tx: EthersBytes::from_static(&[0u8]),
                can_revert: false,
            }],
            validity: None,
            privacy: None,
        };

        let converted = convert_bundle(request)
            .expect("conversion should not fail");
        assert_eq!(converted.protocol_version, AlloyProtocolVersion::Beta1);
        assert_eq!(converted.inclusion.block, 5);
        assert!(converted.inclusion.max_block.is_none());
        assert!(converted.validity.is_none());
        assert!(converted.privacy.is_none());
    }
}
