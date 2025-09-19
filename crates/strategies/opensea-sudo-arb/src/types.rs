use artemis_core::eth::{Address as H160, Hash as H256};
use artemis_core::{
    collectors::{block_collector::NewBlock, opensea_order_collector::OpenseaOrder},
    executors::mempool_types::SubmitTxToMempool,
};
#[cfg(feature = "sdk-ethers")]
use bindings::shared_types::{AdditionalRecipient, BasicOrderParameters};
#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use bindings::{AdditionalRecipient, BasicOrderParameters};
use ethers::types::Chain;

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use alloy_primitives::{
    Address as AlloyAddress, Bytes as AlloyBytes, B256 as AlloyHash, U256 as AlloyU256,
};
use opensea_v2::types::{
    FulfillListingRequest, FulfillListingResponse, Fulfiller, Listing, ProtocolVersion,
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
#[cfg(feature = "sdk-ethers")]
pub fn fulfill_listing_response_to_basic_order_parameters(
    val: &FulfillListingResponse,
) -> BasicOrderParameters {
    let params = &val.fulfillment_data.transaction.input_data.parameters;

    let recipients: Vec<AdditionalRecipient> = params
        .additional_recipients
        .iter()
        .map(|ar| AdditionalRecipient {
            recipient: ar.recipient,
            amount: ar.amount,
        })
        .collect();

    BasicOrderParameters {
        consideration_token: params.consideration_token,
        consideration_identifier: params.consideration_identifier,
        consideration_amount: params.consideration_amount,
        offerer: params.offerer,
        zone: params.zone,
        offer_token: params.offer_token,
        offer_identifier: params.offer_identifier,
        offer_amount: params.offer_amount,
        basic_order_type: params.basic_order_type,
        start_time: params.start_time,
        end_time: params.end_time,
        zone_hash: params.zone_hash.into(),
        salt: params.salt,
        offerer_conduit_key: params.offerer_conduit_key.into(),
        fulfiller_conduit_key: params.fulfiller_conduit_key.into(),
        total_original_additional_recipients: params.total_original_additional_recipients,
        additional_recipients: recipients,
        signature: params.signature.clone(),
    }
}

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
pub fn fulfill_listing_response_to_basic_order_parameters(
    val: &FulfillListingResponse,
) -> BasicOrderParameters {
    let params = &val.fulfillment_data.transaction.input_data.parameters;

    let recipients: Vec<AdditionalRecipient> = params
        .additional_recipients
        .iter()
        .map(|ar| AdditionalRecipient {
            amount: to_alloy_u256(&ar.amount),
            recipient: to_alloy_address(ar.recipient),
        })
        .collect();

    BasicOrderParameters {
        considerationToken: to_alloy_address(params.consideration_token),
        considerationIdentifier: to_alloy_u256(&params.consideration_identifier),
        considerationAmount: to_alloy_u256(&params.consideration_amount),
        offerer: to_alloy_address(params.offerer),
        zone: to_alloy_address(params.zone),
        offerToken: to_alloy_address(params.offer_token),
        offerIdentifier: to_alloy_u256(&params.offer_identifier),
        offerAmount: to_alloy_u256(&params.offer_amount),
        basicOrderType: params.basic_order_type,
        startTime: to_alloy_u256(&params.start_time),
        endTime: to_alloy_u256(&params.end_time),
        zoneHash: to_alloy_hash(params.zone_hash),
        salt: to_alloy_u256(&params.salt),
        offererConduitKey: to_alloy_hash(params.offerer_conduit_key),
        fulfillerConduitKey: to_alloy_hash(params.fulfiller_conduit_key),
        totalOriginalAdditionalRecipients: to_alloy_u256(
            &params.total_original_additional_recipients,
        ),
        additionalRecipients: recipients,
        signature: AlloyBytes::copy_from_slice(params.signature.as_ref()),
    }
}

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
pub(crate) fn to_alloy_address(addr: H160) -> AlloyAddress {
    addr
}

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
pub(crate) fn to_alloy_u256(value: &artemis_core::eth::U256) -> AlloyU256 {
    value.clone()
}

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
pub(crate) fn to_alloy_hash(value: H256) -> AlloyHash {
    value
}

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
pub(crate) fn from_alloy_u256(value: AlloyU256) -> artemis_core::eth::U256 {
    value
}

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
pub(crate) fn from_alloy_address(value: AlloyAddress) -> H160 {
    value
}
