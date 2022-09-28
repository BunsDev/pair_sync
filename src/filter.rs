use crate::dex::{self, Dex};
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
    _usd_threshold: f64,
    provider: Arc<Provider<P>>,
) -> Result<Vec<Pair>, PairSyncError<P>>
where
    P: 'static + JsonRpcClient,
{
    let filtered_pairs = vec![];

    //TODO: get usd weth price

    for pair in pairs {
        //Get token_a/Weth price
        let token_a_weth_pair_address =
            get_token_to_weth_pool(pair.token_a, weth_address, &dexes, provider.clone()).await?;

        //TODO: document behavior where if the pair address can not be found in the dexes you provided,
        //it will be dropped from the final filtered pairs
        if token_a_weth_pair_address == H160::zero() {
            continue;
        }

        //Calculate token_a usd value

        //Get token_b/Weth price

        //Calculate token_b usd value

        //Compare the sum of token_a and token_b usd value against the specified threshold
    }
    Ok(filtered_pairs)
}

//Gets the best token to weth pairing from the dexes provided
async fn get_token_to_weth_pool<P>(
    token_a: H160,
    weth_address: H160,
    dexes: &Vec<Dex>,
    provider: Arc<Provider<P>>,
) -> Result<H160, PairSyncError<P>>
where
    P: 'static + JsonRpcClient,
{
    let mut token_a_weth_pair_address = H160::zero();
    for dex in dexes {
        token_a_weth_pair_address = dex
            .get_pair_with_best_liquidity(token_a, weth_address, provider.clone())
            .await?;
        if token_a_weth_pair_address != H160::zero() {
            break;
        }
    }

    Ok(token_a_weth_pair_address)
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
