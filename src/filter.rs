use crate::dex::{self, Dex, DexType};
use crate::error::PairSyncError;
use crate::pair::Pair;
use ethers::providers::{Http, JsonRpcClient, Provider, ProviderError};
use ethers::types::H160;
use std::{collections::HashSet, sync::Arc};

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

//TODO: write a helperfunction to create a usd_weth_pair pool

//Filter that removes pools with that contain less than a specified usd value
#[allow(dead_code)]
pub async fn filter_pools_below_usd_threshold<P>(
    pairs: Vec<Pair>,
    dexes: Vec<Dex>,
    usd_weth_pair: Pair,
    weth_address: H160,
    usd_threshold: f64,
    provider: Arc<Provider<P>>,
) -> Result<Vec<Pair>, PairSyncError<P>>
where
    P: 'static + JsonRpcClient,
{
    let mut filtered_pairs = vec![];

    //Get price of weth in USD
    let usd_price_per_weth = usd_weth_pair
        .get_price(usd_weth_pair.a_to_b, provider.clone())
        .await?;

    for pair in pairs {
        let (token_a_reserves, token_b_reserves) = if pair.a_to_b {
            (pair.reserve_0, pair.reserve_1)
        } else {
            (pair.reserve_1, pair.reserve_0)
        };

        //Get token_a/Weth price
        let token_a_weth_pair =
            match get_token_to_weth_pool(pair.token_a, weth_address, &dexes, provider.clone())
                .await?
            {
                token_a_weth_pair if !token_a_weth_pair.is_empty() => token_a_weth_pair,
                _ => {
                    //TODO: document behavior where if the pair address can not be found in the dexes you provided,
                    //it will be dropped from the final filtered pairs
                    continue;
                }
            };

        let token_a_price_per_weth = token_a_weth_pair
            .get_price(token_a_weth_pair.token_a == weth_address, provider.clone())
            .await?;

        //Get weth value of token a in pool
        let token_a_weth_value_in_pool =
            ((token_a_reserves * 10u128.pow(pair.token_a_decimals.into())) as f64)
                / token_a_price_per_weth;

        //Calculate token_a usd value
        let token_a_usd_value_in_pool = token_a_weth_value_in_pool * usd_price_per_weth;

        //Get token_b/Weth price
        let token_b_weth_pair =
            match get_token_to_weth_pool(pair.token_b, weth_address, &dexes, provider.clone())
                .await?
            {
                token_a_weth_pair if !token_a_weth_pair.is_empty() => token_a_weth_pair,
                _ => {
                    //TODO: document behavior where if the pair address can not be found in the dexes you provided,
                    //it will be dropped from the final filtered pairs
                    continue;
                }
            };

        let token_b_price_per_weth = token_b_weth_pair
            .get_price(token_b_weth_pair.token_a == weth_address, provider.clone())
            .await?;

        //Get weth value of token a in pool
        let token_b_weth_value_in_pool =
            ((token_b_reserves * 10u128.pow(pair.token_b_decimals.into())) as f64)
                / token_b_price_per_weth;

        //Calculate token_b usd value
        let token_b_usd_value_in_pool = token_b_weth_value_in_pool * usd_price_per_weth;

        //Compare the sum of token_a and token_b usd value against the specified threshold
        let total_usd_value_in_pool = token_a_usd_value_in_pool + token_b_usd_value_in_pool;

        if usd_threshold <= total_usd_value_in_pool {
            filtered_pairs.push(pair);
        }
    }
    Ok(filtered_pairs)
}

//Gets the best token to weth pairing from the dexes provided
async fn get_token_to_weth_pool<P>(
    token_a: H160,
    weth_address: H160,
    dexes: &Vec<Dex>,
    provider: Arc<Provider<P>>,
) -> Result<Pair, PairSyncError<P>>
where
    P: 'static + JsonRpcClient,
{
    let mut token_a_weth_pair = Pair::empty_pair(DexType::UniswapV2);

    for dex in dexes {
        (token_a_weth_pair.pair_address, token_a_weth_pair.fee) = dex
            .get_pair_with_best_liquidity(token_a, weth_address, provider.clone())
            .await?;
        if !token_a_weth_pair.is_empty() {
            break;
        }
    }

    if !token_a_weth_pair.is_empty() {
        token_a_weth_pair.update_a_to_b(provider.clone()).await?;
        token_a_weth_pair.update_reserves(provider).await?;
    }

    Ok(token_a_weth_pair)
}

//Filter that removes pools with that contain less than a specified weth value
//
#[allow(dead_code)]
fn filter_pools_below_weth_threshold(_weth_address: H160, _weth_value_threshold: f64) {}

//Filter to remove tokens that incorporate fees on transfer.
//This filter determines fee on transfer tokens by simulating a transfer and checking if the recieved amount is less
//than the sent amount. It can not be guaranteed that all fee tokens are filtered out. For example,
//if a token has a fee mechanic but the fee is set to 0, this filter will not remove the token.
#[allow(dead_code)]
fn filter_fee_tokens() {}
