use std::collections::BTreeMap;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, BankMsg, Binary, Coin, Deps, DepsMut, Empty, Env, IbcMsg, IbcTimeout, MessageInfo,
    Response, StdResult, Uint128, Uint256, Uint64,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, Rate, ReferenceData, ATTR_ACTION, BAND_CONFIG, ENDPOINT, RATES};
use obi::enc::OBIEncode;

use cw_band::{Input, OracleRequestPacketData};

// WARNING /////////////////////////////////////////////////////////////////////////
// THIS CONTRACT IS AN EXAMPLE HOW TO USE CW_BAND TO WRITE CONTRACT.              //
// PLEASE USE THIS CODE AS THE REFERENCE AND NOT USE THIS CODE IN PRODUCTION.     //
////////////////////////////////////////////////////////////////////////////////////

const E9: Uint64 = Uint64::new(1_000_000_000u64);
const E18: Uint256 = Uint256::from_u128(1_000_000_000_000_000_000u128);

// Version info for migration
const CONTRACT_NAME: &str = "crates.io:band-ibc-price-feed";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    BAND_CONFIG.save(
        deps.storage,
        &Config {
            client_id: msg.client_id,
            manager: msg.manager,
            prices: msg.prices,
            oracle_script_id: msg.oracle_script_id,
            ask_count: msg.ask_count,
            min_count: msg.min_count,
            fee_limit: msg.fee_limit,
            prepare_gas: msg.prepare_gas,
            execute_gas: msg.execute_gas,
            minimum_sources: msg.minimum_sources,
        },
    )?;

    Ok(Response::new().add_attribute("method", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Request { symbols } => try_request(deps, env, info, symbols),
        ExecuteMsg::Withdraw {
            denom,
            amount,
            address,
        } => execute_withdraw(deps, env, info, denom, amount, address),
        ExecuteMsg::UpdateConfig {
            client_id,
            manager,
            prices,
            oracle_script_id,
            ask_count,
            min_count,
            fee_limit,
            prepare_gas,
            execute_gas,
            minimum_sources,
        } => update_config(
            deps,
            info,
            client_id,
            manager,
            prices,
            oracle_script_id,
            ask_count,
            min_count,
            fee_limit,
            prepare_gas,
            execute_gas,
            minimum_sources,
        ),
    }
}

// TODO: Possible features
// - Request fee + Bounty logic to prevent request spam and incentivize relayer
// - Whitelist who can call update price
pub fn try_request(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    symbols: Vec<String>,
) -> Result<Response, ContractError> {
    let endpoint = ENDPOINT.load(deps.storage)?;
    let config = BAND_CONFIG.load(deps.storage)?;
    validate_payment(&config.prices, &info.funds)?;

    let raw_calldata = Input {
        symbols,
        minimum_sources: config.minimum_sources,
    }
    .try_to_vec()
    .map(Binary)
    .map_err(|err| ContractError::CustomError {
        val: err.to_string(),
    })?;

    let packet = OracleRequestPacketData {
        client_id: config.client_id,
        oracle_script_id: config.oracle_script_id,
        calldata: raw_calldata,
        ask_count: config.ask_count,
        min_count: config.min_count,
        prepare_gas: config.prepare_gas,
        execute_gas: config.execute_gas,
        fee_limit: config.fee_limit,
    };

    Ok(Response::new().add_message(IbcMsg::SendPacket {
        channel_id: endpoint.channel_id,
        data: to_binary(&packet)?,
        timeout: IbcTimeout::with_timestamp(env.block.time.plus_seconds(60)),
    }))
}

fn execute_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    denom: String,
    amount: Option<Uint128>,
    address: String,
) -> Result<Response, ContractError> {
    let config = BAND_CONFIG.load(deps.storage)?;

    // if manager set, check the calling address is the authorised multisig otherwise error unauthorised
    let manager = config.manager;
    if info.sender != manager {
        return Err(ContractError::Unauthorized);
    }

    withdraw_unchecked(deps, env, "execute_withdraw", denom, amount, address)
}

fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    client_id: Option<String>,
    manager: Option<String>,
    prices: Option<Vec<Coin>>,
    oracle_script_id: Option<Uint64>,
    ask_count: Option<Uint64>,
    min_count: Option<Uint64>,
    fee_limit: Option<Vec<Coin>>,
    prepare_gas: Option<Uint64>,
    execute_gas: Option<Uint64>,
    minimum_sources: Option<u8>,
) -> Result<Response, ContractError> {
    let mut config = BAND_CONFIG.load(deps.storage)?;

    if info.sender != config.manager {
        return Err(ContractError::Unauthorized);
    }

    if let Some(client_id) = client_id {
        config.client_id = client_id;
    }
    if let Some(manager) = manager {
        config.manager = manager;
    }
    if let Some(prices) = prices {
        config.prices = prices;
    }
    if let Some(oracle_script_id) = oracle_script_id {
        config.oracle_script_id = oracle_script_id;
    }
    if let Some(ask_count) = ask_count {
        config.ask_count = ask_count;
    }
    if let Some(min_count) = min_count {
        config.min_count = min_count;
    }
    if let Some(fee_limit) = fee_limit {
        config.fee_limit = fee_limit;
    }
    if let Some(prepare_gas) = prepare_gas {
        config.prepare_gas = prepare_gas;
    }
    if let Some(execute_gas) = execute_gas {
        config.execute_gas = execute_gas;
    }
    if let Some(minimum_sources) = minimum_sources {
        config.minimum_sources = minimum_sources;
    }

    BAND_CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("method", "update_config"))
}

fn withdraw_unchecked(
    deps: DepsMut,
    env: Env,
    action: &str,
    denom: String,
    amount: Option<Uint128>,
    address: String,
) -> Result<Response, ContractError> {
    let address = deps.api.addr_validate(&address)?;
    let amount: Coin = match amount {
        Some(amount) => Coin { denom, amount },
        None => deps.querier.query_balance(env.contract.address, denom)?,
    };

    let msg = BankMsg::Send {
        to_address: address.into(),
        amount: vec![amount.clone()],
    };
    let res = Response::new()
        .add_message(msg)
        .add_attribute(ATTR_ACTION, action)
        .add_attribute("amount", amount.to_string());
    Ok(res)
}

/// Checks if provided funds are sufficient to pay the price in one of the
/// supported denoms. Payment cannot be split across multiple denoms. Extra funds
/// are ignored.
///
/// When `prices` is an empty list the user cannot pay because there is no possible
/// denomination in which they could do that. This can be desired in case the cantract
/// does not want to accapt any payment (i.e. is closed).
pub fn validate_payment(prices: &[Coin], funds: &[Coin]) -> Result<(), ContractError> {
    if prices.is_empty() {
        return Err(ContractError::NoPaymentOption);
    }

    let prices = BTreeMap::from_iter(prices.iter().map(|c| (c.denom.clone(), c.amount)));
    for fund in funds {
        if let Some(price) = prices.get(&fund.denom) {
            // user can pay in this provided denom
            if fund.amount >= *price {
                return Ok(());
            }
        }
    }
    Err(ContractError::InsufficientPayment)
}

/// this is a no-op
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: Empty) -> StdResult<Response> {
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetRate { symbol } => to_binary(&query_rate(deps, &symbol)?),
        QueryMsg::GetReferenceData { symbol_pair } => {
            to_binary(&query_reference_data(deps, &symbol_pair)?)
        }
        QueryMsg::GetReferenceDataBulk { symbol_pairs } => {
            to_binary(&query_reference_data_bulk(deps, &symbol_pairs)?)
        }
    }
}

fn query_rate(deps: Deps, symbol: &str) -> StdResult<Rate> {
    if symbol == "USD" {
        Ok(Rate::new(E9, Uint64::MAX, Uint64::new(0)))
    } else {
        RATES.load(deps.storage, symbol)
    }
}

fn query_reference_data(deps: Deps, symbol_pair: &(String, String)) -> StdResult<ReferenceData> {
    let base = query_rate(deps, &symbol_pair.0)?;
    let quote = query_rate(deps, &symbol_pair.1)?;

    Ok(ReferenceData::new(
        Uint256::from(base.rate)
            .checked_mul(E18)?
            .checked_div(Uint256::from(quote.rate))?,
        base.resolve_time,
        quote.resolve_time,
    ))
}

fn query_reference_data_bulk(
    deps: Deps,
    symbol_pairs: &[(String, String)],
) -> StdResult<Vec<ReferenceData>> {
    symbol_pairs
        .iter()
        .map(|pair| query_reference_data(deps, pair))
        .collect()
}

// TODO: Writing test
#[cfg(test)]
mod tests {}
