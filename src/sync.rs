use crate::error::PairSyncError;

use super::dex::Dex;
use super::pair::Pair;
use super::throttle::RequestThrottle;
use ethers::{
    providers::{JsonRpcClient, Middleware, Provider, ProviderError},
    types::{BlockNumber, Filter, ValueOrArray, H160, U64},
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::sync::{Arc, Mutex};

//Get all pairs and sync reserve values for each Dex in the `dexes` vec.
pub async fn sync_pairs<P: 'static + JsonRpcClient>(
    dexes: Vec<Dex>,
    provider: Provider<P>,
) -> Result<Vec<Pair>, PairSyncError<P>> {
    //Sync pairs with throttle but set the requests per second limit to 0, disabling the throttle.
    sync_pairs_with_throttle(dexes, provider, 0).await
}

//Get all pairs and sync reserve values for each Dex in the `dexes` vec.
pub async fn sync_pairs_with_throttle<P: 'static + JsonRpcClient>(
    dexes: Vec<Dex>,
    provider: Provider<P>,
    requests_per_second_limit: usize,
) -> Result<Vec<Pair>, PairSyncError<P>> {
    //Initalize a new request throttle
    let request_throttle = Arc::new(Mutex::new(RequestThrottle::new(requests_per_second_limit)));

    let current_block = provider.get_block_number().await?;
    let async_provider = Arc::new(provider);

    let mut handles = vec![];

    //Initialize multi progress bar
    let multi_progress_bar = MultiProgress::new();

    //For each dex supplied, get all pair created events and get reserve values
    for dex in dexes {
        let async_provider = async_provider.clone();
        let request_throttle = request_throttle.clone();
        let progress_bar = multi_progress_bar.add(ProgressBar::new(0));

        handles.push(tokio::spawn(async move {
            progress_bar.set_style(
                ProgressStyle::with_template("{msg} {bar:40.cyan/blue} {pos:>7}/{len:7} Blocks")
                    .unwrap()
                    .progress_chars("##-"),
            );

            let pairs = get_all_pairs(
                dex,
                async_provider.clone(),
                BlockNumber::Number(current_block),
                request_throttle.clone(),
                progress_bar.clone(),
            )
            .await?;

            progress_bar.reset();
            progress_bar.set_style(
                ProgressStyle::with_template("{msg} {bar:40.cyan/blue} {pos:>7}/{len:7} Pairs")
                    .unwrap()
                    .progress_chars("##-"),
            );

            let pairs = get_pair_reserves(
                pairs,
                dex.factory_address,
                async_provider,
                request_throttle,
                progress_bar,
            )
            .await?;

            Ok::<_, PairSyncError<P>>(pairs)
        }));
    }

    //Aggregate the populated pairs from each thread
    let mut aggregated_pairs: Vec<Pair> = vec![];

    for handle in handles {
        match handle.await {
            Ok(sync_result) => aggregated_pairs.extend(sync_result?),
            Err(join_error) => return Err(PairSyncError::JoinError(join_error)),
        }
    }

    //Return the populated aggregated pairs vec
    Ok(aggregated_pairs)
}

//Function to get all pair created events for a given Dex factory address
async fn get_all_pairs<P: 'static + JsonRpcClient>(
    dex: Dex,
    provider: Arc<Provider<P>>,
    current_block: BlockNumber,
    request_throttle: Arc<Mutex<RequestThrottle>>,
    progress_bar: ProgressBar,
) -> Result<Vec<Pair>, PairSyncError<P>> {
    //Define the step for searching a range of blocks for pair created events
    let step = 100000;
    //Unwrap can be used here because the creation block was verified within `Dex::new()`
    let creation_block = dex.creation_block.as_number().unwrap().as_u64();
    let current_block = current_block.as_number().unwrap().as_u64();

    //Initialize the progress bar message
    progress_bar.set_length(current_block - creation_block);
    progress_bar.set_message(format!("Getting all pairs from: {}", dex.factory_address));

    //Init a new vec to keep track of tasks
    let mut handles = vec![];

    //For each block within the range, get all pairs asynchronously
    for from_block in (creation_block..=current_block).step_by(step) {
        let request_throttle = request_throttle.clone();
        let provider = provider.clone();
        let progress_bar = progress_bar.clone();

        //Spawn a new task to get pair created events from the block range
        handles.push(tokio::spawn(async move {
            let mut pairs = vec![];

            //Get pair created event logs within the block range
            let to_block = from_block + step as u64;

            //Update the throttle
            request_throttle.lock().unwrap().increment_or_sleep(1);

            let logs = provider
                .get_logs(
                    &Filter::new()
                        .topic0(ValueOrArray::Value(
                            dex.dex_type.pair_created_event_signature(),
                        ))
                        .from_block(BlockNumber::Number(U64([from_block])))
                        .to_block(BlockNumber::Number(U64([to_block]))),
                )
                .await?;

            //Increment the progres bar by the step
            progress_bar.inc(step as u64);

            //For each pair created log, create a new Pair type and add it to the pairs vec
            for log in logs {
                let pair = dex.new_pair_from_pair_created_event(log, provider.clone());
                if !pair.is_empty() {
                    pairs.push(pair);
                }
            }

            Ok::<Vec<Pair>, ProviderError>(pairs)
        }));
    }

    //Wait for each thread to finish and aggregate the pairs from each Dex into a single aggregated pairs vec
    let mut aggregated_pairs: Vec<Pair> = vec![];
    for handle in handles {
        match handle.await {
            Ok(sync_result) => aggregated_pairs.extend(sync_result?),

            Err(join_error) => return Err(PairSyncError::JoinError(join_error)),
        }
    }
    Ok(aggregated_pairs)
}

//Function to get reserves for each pair in the `pairs` vec.
async fn get_pair_reserves<P: 'static + JsonRpcClient>(
    pairs: Vec<Pair>,
    dex_factory_address: H160,
    provider: Arc<Provider<P>>,
    request_throttle: Arc<Mutex<RequestThrottle>>,
    progress_bar: ProgressBar,
) -> Result<Vec<Pair>, PairSyncError<P>> {
    //Initialize a vec to track each async task.
    let mut handles: Vec<tokio::task::JoinHandle<Result<Pair, _>>> = vec![];

    //Initialize the progress bar message
    progress_bar.set_length(pairs.len() as u64);
    progress_bar.set_message(format!(
        "Syncing reserves for pairs from: {}",
        dex_factory_address
    ));

    //For each pair in the pairs vec, get the reserves asyncrhonously
    for mut pair in pairs {
        let request_throttle = request_throttle.clone();
        let provider = provider.clone();
        let progress_bar = progress_bar.clone();

        //Spawn a new thread to get the reserves for the pair
        handles.push(tokio::spawn(async move {
            progress_bar.inc(1);

            //Get the pair reserves
            //If the pair is uniswapv3, two rpc calls are made to initialize reserves
            //Because of this, the throttle increments by two to be conservative
            request_throttle.lock().unwrap().increment_or_sleep(2);
            (pair.reserve_0, pair.reserve_1) = pair.get_reserves(provider.clone()).await?;

            // Make a call to get token0 to initialize a_to_b
            request_throttle.lock().unwrap().increment_or_sleep(1);
            let token_0 = pair.get_token_0(provider.clone()).await?;

            //Update a_to_b
            if token_0 != H160::zero() {
                pair.a_to_b = pair.token_a == token_0;
            } else {
                pair = Pair::empty_pair(pair.dex_type)
            };

            //Update token decimals
            pair.update_token_decimals(provider.clone()).await?;

            Ok::<Pair, PairSyncError<P>>(pair)
        }));
    }

    //Create a new vec to aggregate the pairs and populate the vec.
    let mut updated_pairs: Vec<Pair> = vec![];
    for handle in handles {
        match handle.await {
            Ok(sync_result) => updated_pairs.push(sync_result?),

            Err(join_error) => return Err(PairSyncError::JoinError(join_error)),
        }
    }

    //Return the vec of pairs with updated reserve values
    Ok(updated_pairs)
}
