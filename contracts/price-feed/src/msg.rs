use crate::state::{Rate, ReferenceData};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Uint128, Uint64};

#[cw_serde]
pub struct InstantiateMsg {
    // A unique ID for the oracle request
    pub client_id: String,
    // Manager of contract
    pub manager: String,
    /// The prices of a oracle. List is to be interpreted as oneof,
    /// i.e. payment must be paid in one of those denominations.
    /// If this list is empty, the user cannot pay. This can be used to put the
    /// contract out of service.
    pub prices: Vec<Coin>,
    // The oracle script ID to query
    pub oracle_script_id: Uint64,
    // The number of validators that are requested to respond
    pub ask_count: Uint64,
    // The minimum number of validators that need to respond
    pub min_count: Uint64,
    // The maximum amount of band in uband to be paid to the data source providers
    // e.g. vec![Coin::new(100, "uband")]
    pub fee_limit: Vec<Coin>,
    // Amount of gas to pay to prepare raw requests
    pub prepare_gas: Uint64,
    // Amount of gas reserved for execution
    pub execute_gas: Uint64,
    // Minimum number of sources required to return a successful response
    pub minimum_sources: u8,
}

#[cw_serde]
pub enum ExecuteMsg {
    Request {
        symbols: Vec<String>,
    },
    /// Withdraw the given amount to the withdrawal address.
    ///
    /// Only the manager address can do that.
    Withdraw {
        denom: String,
        /// The amount of tokens to withdraw.
        /// If None, withdraw all available balance of the given denom.
        amount: Option<Uint128>,
        address: String,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Rate)]
    // Returns the RefData of a given symbol
    GetRate {
        // Symbol to query
        symbol: String,
    },
    #[returns(ReferenceData)]
    // Returns the ReferenceData of a given asset pairing
    GetReferenceData {
        // Symbol pair to query where:
        // symbol_pair := (base_symbol, quote_symbol)
        // e.g. BTC/USD ≡ ("BTC", "USD")
        symbol_pair: (String, String),
    },
    #[returns(Vec<ReferenceData>)]
    // Returns the ReferenceDatas of the given asset pairings
    GetReferenceDataBulk {
        // Vector of Symbol pair to query
        // e.g. <BTC/USD ETH/USD, BAND/BTC> ≡ <("BTC", "USD"), ("ETH", "USD"), ("BAND", "BTC")>
        symbol_pairs: Vec<(String, String)>,
    },
}
