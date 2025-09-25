#![allow(clippy::all)]

pub use generated::*;

pub mod bytecode;

#[allow(clippy::all)]
mod generated {
    use alloy_sol_types::sol;

    sol! {
        struct AdditionalRecipient {
            uint256 amount;
            address recipient;
        }

        struct BasicOrderParameters {
            address considerationToken;
            uint256 considerationIdentifier;
            uint256 considerationAmount;
            address payable offerer;
            address zone;
            address offerToken;
            uint256 offerIdentifier;
            uint256 offerAmount;
            uint8 basicOrderType;
            uint256 startTime;
            uint256 endTime;
            bytes32 zoneHash;
            uint256 salt;
            bytes32 offererConduitKey;
            bytes32 fulfillerConduitKey;
            uint256 totalOriginalAdditionalRecipients;
            AdditionalRecipient[] additionalRecipients;
            bytes signature;
        }

        struct SellQuote {
            bool quoteAvailable;
            address nftAddress;
            uint256 price;
        }

        #[sol(rpc)]
        contract SudoPairQuoter {
            function getSellQuote(address pool_address) external view returns (SellQuote sell_quote);
            function getMultipleSellQuotes(address[] pool_addresses) external view returns (SellQuote[] sell_quotes);
        }

        #[sol(rpc)]
        contract SudoOpenseaArb {
            function executeArb(
                BasicOrderParameters basicOrder,
                uint256 paymentValue,
                address sudo_pool
            ) external;
        }

        #[sol(rpc)]
        contract LSSVMPairFactory {
            event NewPair(address poolAddress);
        }

        #[sol(rpc)]
        contract LSSVMPair {
            event SwapNFTInPair();
            event SpotPriceUpdate(uint128 newSpotPrice);
            event TokenWithdrawal(uint256 amount);
        }
    }
}
