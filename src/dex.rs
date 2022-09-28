use std::{str::FromStr, sync::Arc};

use ethers::{
    prelude::ContractError,
    providers::{JsonRpcClient, Middleware, Provider, ProviderError},
    types::{Address, BlockNumber, Log, H160, H256, U256},
};

use crate::{abi, error::PairSyncError, pair::Pair};

#[derive(Debug, Clone, Copy)]
pub struct Dex {
    pub factory_address: H160,
    pub dex_type: DexType,
    pub creation_block: BlockNumber,
}

#[derive(Debug, Clone, Copy)]
pub enum DexType {
    UniswapV2,
    UniswapV3,
}

impl Dex {
    pub fn new(factory_address: &str, dex_type: DexType, creation_block: u64) -> Dex {
        Dex {
            factory_address: H160::from_slice(factory_address.as_bytes()),
            dex_type,
            creation_block: BlockNumber::Number(creation_block.into()),
        }
    }

    //TODO: rename this to be specific to what it needs to do
    //This should get the pair with the best liquidity from the dex variant.
    //If univ2, there will only be one pool, if univ3 there will be multiple
    pub async fn get_pair_with_best_liquidity<P>(
        &self,
        token_a: H160,
        token_b: H160,
        provider: Arc<Provider<P>>,
    ) -> Result<H160, PairSyncError<P>>
    where
        P: 'static + JsonRpcClient,
    {
        match self.dex_type {
            DexType::UniswapV2 => {
                let uniswap_v2_factory =
                    abi::IUniswapV2Factory::new(self.factory_address, provider);

                Ok(uniswap_v2_factory.get_pair(token_a, token_b).call().await?)
            }

            DexType::UniswapV3 => {
                let uniswap_v3_factory =
                    abi::IUniswapV3Factory::new(self.factory_address, provider.clone());

                let mut best_liquidity = 0;
                let mut best_pool_address = H160::zero();

                for fee in [100, 300, 500, 1000] {
                    let pool_address = uniswap_v3_factory
                        .get_pool(token_a, token_b, fee)
                        .call()
                        .await?;

                    let uniswap_v3_pool = abi::IUniswapV3Pool::new(pool_address, provider.clone());

                    let liquidity = uniswap_v3_pool.liquidity().call().await?;
                    if best_liquidity < liquidity {
                        best_liquidity = liquidity;
                        best_pool_address = pool_address;
                    }
                }

                Ok(best_pool_address)
            }
        }
    }

    pub fn new_pair_from_pair_created_event<P>(&self, log: Log, provider: Arc<Provider<P>>) -> Pair
    where
        P: JsonRpcClient,
    {
        match self.dex_type {
            DexType::UniswapV2 => {
                let uniswap_v2_factory =
                    abi::IUniswapV2Factory::new(self.factory_address, provider);

                let (token_a, token_b, pair_address, _) =
                    match uniswap_v2_factory.decode_event::<(Address, Address, Address, U256)>(
                        "PairCreated",
                        log.topics,
                        log.data,
                    ) {
                        Ok(result) => result,
                        Err(_) => {
                            //If there was an abi error, continue without adding the pair
                            return Pair::empty_pair(self.dex_type);
                        }
                    };

                Pair {
                    dex_type: DexType::UniswapV2,
                    pair_address,
                    token_a,
                    token_b,
                    //Initialize the following variables as zero values
                    //They will be populated when getting pair reserves
                    token_a_decimals: 0,
                    token_b_decimals: 0,
                    a_to_b: false,
                    reserve_0: 0,
                    reserve_1: 0,
                    fee: 300,
                }
            }
            DexType::UniswapV3 => {
                let uniswap_v3_factory =
                    abi::IUniswapV3Factory::new(self.factory_address, provider);

                let (token_a, token_b, fee, _, pair_address) = match uniswap_v3_factory
                    .decode_event::<(Address, Address, u32, u128, Address)>(
                        "PoolCreated",
                        log.topics,
                        log.data,
                    ) {
                    Ok(result) => result,
                    Err(_) => {
                        //If there was an abi error, continue without adding the pair
                        return Pair::empty_pair(self.dex_type);
                    }
                };

                Pair {
                    dex_type: DexType::UniswapV3,

                    pair_address,
                    token_a,
                    token_b,
                    //Initialize the following variables as zero values
                    //They will be populated when getting pair reserves
                    token_a_decimals: 0,
                    token_b_decimals: 0,
                    a_to_b: false,
                    reserve_0: 0,
                    reserve_1: 0,
                    fee,
                }
            }
        }
    }
}

impl DexType {
    pub fn pair_created_event_signature(&self) -> H256 {
        match self {
            DexType::UniswapV2 => {
                H256::from_str("0x0d3648bd0f6ba80134a33ba9275ac585d9d315f0ad8355cddefde31afa28d0e9")
                    .unwrap()
            }
            DexType::UniswapV3 => {
                H256::from_str("0x783cca1c0412dd0d695e784568c96da2e9c22ff989357a2e8b1d9b2b4e6b7118")
                    .unwrap()
            }
        }
    }

    pub async fn get_reserves<P>(
        &self,
        token_a: H160,
        token_b: H160,
        pair_address: H160,
        provider: Arc<Provider<P>>,
    ) -> Result<(u128, u128), ProviderError>
    where
        P: JsonRpcClient,
    {
        match self {
            DexType::UniswapV2 => {
                //Initialize a new instance of the Pool
                let v2_pair = abi::IUniswapV2Pair::new(pair_address, provider);

                // Make a call to get the reserves
                let (reserve_0, reserve_1, _) = match v2_pair.get_reserves().call().await {
                    Ok(result) => result,

                    Err(contract_error) => match contract_error {
                        ContractError::ProviderError(provider_error) => return Err(provider_error),

                        _ => (0, 0, 0),
                    },
                };

                Ok((reserve_0, reserve_1))
            }
            DexType::UniswapV3 => {
                //Initialize a new instance of token_a
                let token_a = abi::IErc20::new(token_a, provider.clone());

                // Make a call to get the Pool's balance of token_a
                let reserve_0 = match token_a.balance_of(pair_address).call().await {
                    Ok(result) => result,
                    Err(contract_error) => match contract_error {
                        ContractError::ProviderError(provider_error) => return Err(provider_error),
                        _ => {
                            return Ok((0, 0));
                        }
                    },
                };

                //Initialize a new instance of token_b
                let token_b = abi::IErc20::new(token_b, provider);

                // Make a call to get the Pool's balance of token_b
                let reserve_1 = match token_b.balance_of(pair_address).call().await {
                    Ok(result) => result,
                    Err(contract_error) => match contract_error {
                        ContractError::ProviderError(provider_error) => return Err(provider_error),
                        _ => {
                            return Ok((0, 0));
                        }
                    },
                };

                Ok((reserve_0.as_u128(), reserve_1.as_u128()))
            }
        }
    }

    pub async fn get_token_0<P>(
        &self,
        pair_address: H160,
        provider: Arc<Provider<P>>,
    ) -> Result<H160, ProviderError>
    where
        P: JsonRpcClient,
    {
        match self {
            DexType::UniswapV2 => {
                //Initialize a new instance of the Pool
                let v2_pair = abi::IUniswapV2Pair::new(pair_address, provider);

                // Make a call to get token0 to initialize a_to_b
                let token0 = match v2_pair.token_0().call().await {
                    Ok(result) => result,
                    Err(contract_error) => match contract_error {
                        ContractError::ProviderError(provider_error) => return Err(provider_error),
                        _ => {
                            return Ok(H160::zero());
                        }
                    },
                };

                Ok(token0)
            }
            DexType::UniswapV3 => {
                //Initialize a new instance of the Pool
                let v3_pool = abi::IUniswapV3Pool::new(pair_address, provider);

                // Make a call to get token0 and initialize a_to_b
                let token0 = match v3_pool.token_0().call().await {
                    Ok(result) => result,
                    Err(contract_error) => match contract_error {
                        ContractError::ProviderError(provider_error) => return Err(provider_error),
                        _ => {
                            return Ok(H160::zero());
                        }
                    },
                };

                Ok(token0)
            }
        }
    }
}
