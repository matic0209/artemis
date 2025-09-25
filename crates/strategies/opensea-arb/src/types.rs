use artemis_core::eth::{Address as H160, Hash as H256};
use artemis_core::{
    collectors::{block_collector::NewBlock, opensea_order_collector::OpenseaOrder},
    executors::mempool_types::SubmitTxToMempool,
};
use bindings::{AdditionalRecipient, BasicOrderParameters};
use alloy_primitives::Bytes as AlloyBytes;
use opensea_v2::types::{
    Chain, FulfillListingRequest, FulfillListingResponse, Fulfiller, Listing, ProtocolVersion,
};

/// Core Event enum for the current strategy.
#[derive(Debug, Clone)]
pub enum Event {
    NewBlock(NewBlock),
    OpenseaOrder(Box<OpenseaOrder>),
}

/// Core Action enum for the current strategy.
#[derive(Debug, Clone)]
pub enum Action {
    SubmitTx(SubmitTxToMempool),
}

/// Configuration for variables we need to pass to the strategy.
#[derive(Debug, Clone)]
pub struct Config {
    pub arb_contract_address: H160,
    pub bid_percentage: u64,
}

/// Convenience function to convert a hash to a fulfill listing request
pub fn hash_to_fulfill_listing_request(hash: H256) -> FulfillListingRequest {
    FulfillListingRequest {
        listing: Listing {
            hash,
            chain: Chain::Mainnet,
            protocol_version: ProtocolVersion::V1_5,
        },
        fulfiller: Fulfiller {
            address: H160::default(),
        },
    }
}

/// Convenience function to convert a fulfill listing response to basic order parameters
pub fn fulfill_listing_response_to_basic_order_parameters(
    val: &FulfillListingResponse,
) -> BasicOrderParameters {
    let params = &val.fulfillment_data.transaction.input_data.parameters;

    let recipients: Vec<AdditionalRecipient> = params
        .additional_recipients
        .iter()
        .map(|ar| AdditionalRecipient {
            amount: ar.amount,
            recipient: ar.recipient,
        })
        .collect();

    BasicOrderParameters {
        considerationToken: params.consideration_token,
        considerationIdentifier: params.consideration_identifier,
        considerationAmount: params.consideration_amount,
        offerer: params.offerer,
        zone: params.zone,
        offerToken: params.offer_token,
        offerIdentifier: params.offer_identifier,
        offerAmount: params.offer_amount,
        basicOrderType: params.basic_order_type,
        startTime: params.start_time,
        endTime: params.end_time,
        zoneHash: params.zone_hash,
        salt: params.salt,
        offererConduitKey: params.offerer_conduit_key,
        fulfillerConduitKey: params.fulfiller_conduit_key,
        totalOriginalAdditionalRecipients: params.total_original_additional_recipients,
        additionalRecipients: recipients,
        signature: AlloyBytes::copy_from_slice(params.signature.as_ref()),
    }
}

/// Convert artemis Address (H160) to alloy Address
pub fn to_alloy_address(addr: H160) -> alloy_primitives::Address {
    alloy_primitives::Address::from(addr.0)
}

/// Convert alloy Address to artemis Address (H160)
pub fn from_alloy_address(addr: alloy_primitives::Address) -> H160 {
    H160(addr.0)
}

/// Convert artemis U256 to alloy U256
pub fn to_alloy_u256(val: &artemis_core::eth::U256) -> alloy_primitives::U256 {
    let bytes: [u8; 32] = val.to_be_bytes();
    alloy_primitives::U256::from_be_bytes(bytes)
}

/// Convert alloy U256 to artemis U256
pub fn from_alloy_u256(val: alloy_primitives::U256) -> artemis_core::eth::U256 {
    let bytes: [u8; 32] = val.to_be_bytes();
    artemis_core::eth::U256::from_be_bytes(bytes)
}
