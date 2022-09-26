use std::{collections::HashSet, sync::Arc};

use super::abi;
use crate::dex::Dex;
use crate::pair::Pair;
use ethers::prelude::ContractError;
use ethers::providers::{Http, Provider, ProviderError};
use ethers::{prelude::abigen, types::H160};

//Filters out pairs where the blacklisted address is the token_a address or token_b address
pub fn filter_blacklisted_tokens(pairs: Vec<Pair>, blacklisted_addresses: Vec<H160>) -> Vec<Pair> {
    let mut filtered_pairs = vec![];
    let blacklist: HashSet<H160> = blacklisted_addresses.into_iter().collect();

    for pair in pairs {
        if !blacklist.contains(&pair.token_a) || !blacklist.contains(&pair.token_b) {
            filtered_pairs.push(pair);
        }
    }

    filtered_pairs
}

//Filters out pairs where the blacklisted address is the pair address
pub fn filter_blacklisted_pools(pairs: Vec<Pair>, blacklisted_addresses: Vec<H160>) -> Vec<Pair> {
    let mut filtered_pairs = vec![];
    let blacklist: HashSet<H160> = blacklisted_addresses.into_iter().collect();

    for pair in pairs {
        if !blacklist.contains(&pair.pair_address) {
            filtered_pairs.push(pair);
        }
    }

    filtered_pairs
}

//Filters out pairs where the blacklisted address is the pair address, token_a address or token_b address
pub fn filter_blacklisted_addresses(
    pairs: Vec<Pair>,
    blacklisted_addresses: Vec<H160>,
) -> Vec<Pair> {
    let mut filtered_pairs = vec![];
    let blacklist: HashSet<H160> = blacklisted_addresses.into_iter().collect();

    for pair in pairs {
        if !blacklist.contains(&pair.pair_address)
            || !blacklist.contains(&pair.token_a)
            || !blacklist.contains(&pair.token_b)
        {
            filtered_pairs.push(pair);
        }
    }

    filtered_pairs
}

//Filter that removes pools with that contain less than a specified usd value
#[allow(dead_code)]
pub async fn filter_pools_below_usd_threshold(
    pairs: Vec<Pair>,
    dexes: Vec<Dex>,
    usd_address: H160,
    weth_address: H160,
    usd_threshold: f64,
    provider: Arc<Provider<Http>>,
) -> Result<Vec<Pair>, ProviderError> {
    let mut filtered_pairs = vec![];

    //Get USD/Weth price
    // let usd_weth_pair = abi::IUniswapV2Pair::new(usd_weth_pair_address, provider);

    // dexes[0].getReserves();

    // let (reserve_0, reserve_1, _) = match usd_weth_pair.get_reserves().call().await {
    //     Ok(result) => result,
    //     Err(contract_error) => match contract_error {
    //         ContractError::ProviderError(provider_error) => return Err(provider_error),
    //         other => {
    //             panic!(
    //                 "Error when getting USD/Weth reserves for filter USD Threshold filter: {}",
    //                 other.to_string()
    //             )
    //         }
    //     },
    // };

    for pair in pairs {

        //Get token_a/Weth price

        //Calculate token_a usd value

        //Get token_b/Weth price

        //Calculate token_b usd value

        //Compare the sum of token_a and token_b usd value against the specified threshold
    }
    Ok(filtered_pairs)
}

//Filter that removes pools with that contain less than a specified weth value
//
#[allow(dead_code)]
fn filter_pools_below_weth_threshold(weth_address: H160, weth_value_threshold: f64) {}

//Filter to remove tokens that incorporate fees on transfer.
//This filter determines fee on transfer tokens by simulating a transfer and checking if the recieved amount is less
//than the sent amount. It can not be guaranteed that all fee tokens are filtered out. For example,
//if a token has a fee mechanic but the fee is set to 0, this filter will not remove the token.
#[allow(dead_code)]
fn filter_fee_tokens() {}
