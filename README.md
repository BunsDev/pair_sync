# pair_sync

A simple library to get all pairs from any supported Dex and sync reserves.

- [Crates.io](https://crates.io/crates/pair_sync)
- [Documentation in progress](https://docs.rs/pair_sync/0.1.0/pair_sync/)


Filename: examples/sync-pairs.rs
```rust
use ethers::providers::{Http, Provider, ProviderError};
use pair_sync::{dex::Dex, dex::DexType, sync};

#[tokio::main]
async fn main() -> Result<(), ProviderError> {
    //Add rpc endpoint here:
    let rpc_endpoint = "";
    let provider = Provider::<Http>::try_from(rpc_endpoint).unwrap();

    let mut dexes = vec![];

    //Add UniswapV2
    dexes.push(Dex::new(
        //Specify the factory address
        H160::from_str("0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f").unwrap(),
        //Specify the dex variant
        DexType::UniswapV2,
        //Specify the factory contract's creation block number
        2638438,
    ));

    //Add Sushiswap
    dexes.push(Dex::new(
        H160::from_str("0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac").unwrap(),
        DexType::UniswapV2,
        10794229,
    ));

    //Add UniswapV3
    dexes.push(Dex::new(
        H160::from_str("0x1F98431c8aD98523631AE4a59f267346ea31F984").unwrap(),
        DexType::UniswapV3,
        12369621,
    ));

    //Sync pairs
    let pools: Vec<Pool> = sync::sync_pairs(dexes, provider).await?;

    Ok(())
}
```

## Supported Dexes

| Dex | Status |
|----------|------|
| UniswapV2 variants  | ✅||
| UniswapV3  | ✅||


## Running Examples

To run any of the examples, supply your node endpoint to the endpoint variable in each example file. For example in `sync-pairs.rs`:

```rust
    //Add rpc endpoint here:
    let rpc_endpoint = "";
```

Once you have supplied a node endpoint, you can simply run `cargo run --example <example_name>`.


## Filters

#### `filter_blacklisted_tokens`
- Removes any pair from a `Vec<Pair>` where either `token_a` or `token_b` matches a blacklisted address.

#### `filter_blacklisted_pools`
- Removes any pair from a `Vec<Pair>` where the `pair_address` matches a blacklisted address.

#### `filter_blacklisted_addresses`
- Removes any pair from a `Vec<Pair>` where either `token_a`, `token_b` or the `pair_address` matches a blacklisted address.


## Upcoming Filters

#### `filter_pools_below_usd_threshold`
- Removes any pair where the USD value of the pool is below the specified USD threshold.

#### `filter_pools_below_weth_threshold`
- Removes any pair where the USD value of the pool is below the specified WETH threshold.

#### `filter_fee_tokens`
- Removes any pair where  where either `token_a` or `token_b` is a token with fee on transfer.



