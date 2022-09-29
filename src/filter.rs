use crate::dex::{Dex, DexType};
use crate::error::PairSyncError;
use crate::pair::Pair;
use crate::throttle::RequestThrottle;
use ethers::providers::{JsonRpcClient, Provider};
use ethers::types::H160;
use std::collections::HashMap;
use std::sync::Mutex;
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
    Ok(filter_pools_below_usd_threshold_with_throttle(
        pairs,
        dexes,
        usd_weth_pair,
        weth_address,
        usd_threshold,
        provider,
        0,
    )
    .await?)
}

//Filter that removes pools with that contain less than a specified usd value
pub async fn filter_pools_below_usd_threshold_with_throttle<P>(
    pairs: Vec<Pair>,
    dexes: Vec<Dex>,
    usd_weth_pair: Pair,
    weth_address: H160,
    usd_threshold: f64,
    provider: Arc<Provider<P>>,
    requests_per_second_limit: usize,
) -> Result<Vec<Pair>, PairSyncError<P>>
where
    P: 'static + JsonRpcClient,
{
    //Init a new vec to hold the filtered pairs
    let mut filtered_pairs = vec![];

    let request_throttle = Arc::new(Mutex::new(RequestThrottle::new(requests_per_second_limit)));

    //Get price of weth in USD
    let usd_price_per_weth = usd_weth_pair
        .get_price(usd_weth_pair.a_to_b, provider.clone())
        .await?;

    //Initialize a Hashmap to keep track of token/weth prices already found to avoid unnecessary calls to the node
    let token_weth_prices: Arc<Mutex<HashMap<H160, f64>>> = Arc::new(Mutex::new(HashMap::new()));
    let mut handles = vec![];
    //For each pair, check if the usd value meets the specified threshold
    for pair in pairs {
        let token_weth_prices = token_weth_prices.clone();
        let request_throttle = request_throttle.clone();
        let provider = provider.clone();
        let dexes = dexes.clone();

        handles.push(tokio::spawn(async move {
            let (token_a_reserves, token_b_reserves) = if pair.a_to_b {
                (pair.reserve_0, pair.reserve_1)
            } else {
                (pair.reserve_1, pair.reserve_0)
            };

            let token_a_price_per_weth = token_weth_prices
                .lock()
                .unwrap()
                .get(&pair.token_a)
                .map(|price| price.to_owned());

            let token_a_price_per_weth = match token_a_price_per_weth {
                Some(price) => price,
                None => {
                    request_throttle.lock().unwrap().increment_or_sleep(1);
                    let price = get_price_of_token_per_weth(
                        pair.token_a,
                        weth_address,
                        &dexes,
                        provider.clone(),
                    )
                    .await?;

                    token_weth_prices
                        .lock()
                        .unwrap()
                        .insert(pair.token_a, price);

                    price
                }
            };

            //Get weth value of token a in pool
            let token_a_weth_value_in_pool =
                ((token_a_reserves * 10u128.pow(pair.token_a_decimals.into())) as f64)
                    / token_a_price_per_weth;

            //Calculate token_a usd value
            let token_a_usd_value_in_pool = token_a_weth_value_in_pool * usd_price_per_weth;

            let token_b_price_per_weth = token_weth_prices
                .lock()
                .unwrap()
                .get(&pair.token_b)
                .map(|price| price.to_owned());

            let token_b_price_per_weth = match token_b_price_per_weth {
                Some(price) => price.to_owned(),
                None => {
                    request_throttle.lock().unwrap().increment_or_sleep(1);
                    let price = get_price_of_token_per_weth(
                        pair.token_b,
                        weth_address,
                        &dexes,
                        provider.clone(),
                    )
                    .await?;

                    token_weth_prices
                        .lock()
                        .unwrap()
                        .insert(pair.token_b, price);

                    price
                }
            };

            //Get weth value of token a in pool
            let token_b_weth_value_in_pool =
                ((token_b_reserves * 10u128.pow(pair.token_b_decimals.into())) as f64)
                    / token_b_price_per_weth;

            //Calculate token_b usd value
            let token_b_usd_value_in_pool = token_b_weth_value_in_pool * usd_price_per_weth;

            //Compare the sum of token_a and token_b usd value against the specified threshold
            let total_usd_value_in_pool = token_a_usd_value_in_pool + token_b_usd_value_in_pool;

            Ok::<_, PairSyncError<P>>((total_usd_value_in_pool, pair))
        }));
    }

    for handle in handles {
        match handle.await {
            Ok(filter_result) => match filter_result {
                Ok((total_usd_value_in_pool, pool)) => {
                    if usd_threshold <= total_usd_value_in_pool {
                        filtered_pairs.push(pool);
                    }
                }
                Err(pair_sync_error) => match pair_sync_error {
                    PairSyncError::PairDoesNotExistInDexes(_, _) => {}
                    _ => return Err(pair_sync_error),
                },
            },

            Err(join_error) => return Err(PairSyncError::JoinError(join_error)),
        }
    }

    Ok(filtered_pairs)
}

async fn get_price_of_token_per_weth<P: 'static + JsonRpcClient>(
    token_address: H160,
    weth_address: H160,
    dexes: &Vec<Dex>,
    provider: Arc<Provider<P>>,
) -> Result<f64, PairSyncError<P>> {
    if token_address == weth_address {
        return Ok(1.0);
    }

    //Get token_a/weth price
    let token_a_weth_pair =
        get_token_to_weth_pool(token_address, weth_address, &dexes, provider.clone()).await?;

    let token_a_price_per_weth = token_a_weth_pair
        .get_price(token_a_weth_pair.token_a == weth_address, provider.clone())
        .await?;

    Ok(token_a_price_per_weth)
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
    } else {
        return Err(PairSyncError::PairDoesNotExistInDexes(
            token_a,
            weth_address,
        ));
    }

    Ok(token_a_weth_pair)
}

//Filter that removes pools with that contain less than a specified weth value
//
pub async fn filter_pools_below_weth_threshold<P>(
    pairs: Vec<Pair>,
    dexes: Vec<Dex>,
    weth_address: H160,
    weth_threshold: f64,
    provider: Arc<Provider<P>>,
) -> Result<Vec<Pair>, PairSyncError<P>>
where
    P: 'static + JsonRpcClient,
{
    let mut filtered_pairs = vec![];
    Ok(filtered_pairs)
}

//Filter to remove tokens that incorporate fees on transfer.
//This filter determines fee on transfer tokens by simulating a transfer and checking if the recieved amount is less
//than the sent amount. It can not be guaranteed that all fee tokens are filtered out. For example,
//if a token has a fee mechanic but the fee is set to 0, this filter will not remove the token.
#[allow(dead_code)]
fn filter_fee_tokens() {}
