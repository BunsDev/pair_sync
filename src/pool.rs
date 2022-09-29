use std::sync::Arc;

use crate::{abi, dex::DexType, error::PairSyncError};
use ethers::{
    providers::{JsonRpcClient, Provider, ProviderError},
    types::H160,
};

#[derive(Debug)]
pub struct Pool {
    pub address: H160,
    pub token_a: H160,
    pub token_a_decimals: u8,
    pub token_b: H160,
    pub token_b_decimals: u8,
    pub a_to_b: bool,
    pub reserve_0: u128,
    pub reserve_1: u128,
    pub fee: u32,
    pub dex_type: DexType,
}

impl Pool {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        address: H160,
        token_a: H160,
        token_a_decimals: u8,
        token_b: H160,
        token_b_decimals: u8,
        a_to_b: bool,
        reserve_0: u128,
        reserve_1: u128,
        fee: u32,
        dex_type: DexType,
    ) -> Pool {
        Pool {
            address,
            token_a,
            token_a_decimals,
            token_b,
            token_b_decimals,
            a_to_b,
            reserve_0,
            reserve_1,
            fee,
            dex_type,
        }
    }

    pub fn empty_pool(dex_type: DexType) -> Pool {
        Pool {
            address: H160::zero(),
            token_a: H160::zero(),
            token_a_decimals: 0,
            token_b: H160::zero(),
            token_b_decimals: 0,
            a_to_b: false,
            reserve_0: 0,
            reserve_1: 0,
            fee: 0,
            dex_type,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.token_a == H160::zero()
    }

    pub fn reserves_are_zero(&self) -> bool {
        self.reserve_0 == 0 && self.reserve_1 == 0
    }

    pub async fn get_reserves<P>(
        &self,
        provider: Arc<Provider<P>>,
    ) -> Result<(u128, u128), ProviderError>
    where
        P: JsonRpcClient,
    {
        self.dex_type
            .get_reserves(self.token_a, self.token_b, self.address, provider)
            .await
    }

    pub async fn update_reserves<P>(
        &mut self,
        provider: Arc<Provider<P>>,
    ) -> Result<(), ProviderError>
    where
        P: JsonRpcClient,
    {
        let (reserve0, reserve1) = self
            .dex_type
            .get_reserves(self.token_a, self.token_b, self.address, provider)
            .await?;

        self.reserve_0 = reserve0;
        self.reserve_1 = reserve1;

        Ok(())
    }

    pub async fn get_token_0<P>(&self, provider: Arc<Provider<P>>) -> Result<H160, ProviderError>
    where
        P: JsonRpcClient,
    {
        self.dex_type.get_token_0(self.address, provider).await
    }

    pub async fn update_a_to_b<P>(
        &mut self,
        provider: Arc<Provider<P>>,
    ) -> Result<(), ProviderError>
    where
        P: JsonRpcClient,
    {
        let token0 = self.dex_type.get_token_0(self.address, provider).await?;

        self.a_to_b = token0 == self.token_a;

        Ok(())
    }

    pub async fn get_price<P>(
        &self,
        a_per_b: bool,
        provider: Arc<Provider<P>>,
    ) -> Result<f64, PairSyncError<P>>
    where
        P: JsonRpcClient,
    {
        let (reserve_0, reserve_1) = self.get_reserves(provider.clone()).await?;

        let reserve_0 = (reserve_0 * 10u128.pow(self.token_a_decimals.into())) as f64;
        let reserve_1 = (reserve_1 * 10u128.pow(self.token_b_decimals.into())) as f64;

        match self.dex_type {
            DexType::UniswapV2 => {
                if self.a_to_b {
                    if a_per_b {
                        Ok(reserve_0 / reserve_1)
                    } else {
                        Ok(reserve_1 / reserve_0)
                    }
                } else if a_per_b {
                    Ok(reserve_1 / reserve_0)
                } else {
                    Ok(reserve_0 / reserve_1)
                }
            }

            DexType::UniswapV3 => {
                //TODO: double check this
                if self.a_to_b {
                    if a_per_b {
                        Ok(reserve_0 / reserve_1)
                    } else {
                        Ok(reserve_1 / reserve_0)
                    }
                } else if a_per_b {
                    Ok(reserve_1 / reserve_0)
                } else {
                    Ok(reserve_0 / reserve_1)
                }
            }
        }
    }

    pub async fn update_token_decimals<P: 'static + JsonRpcClient>(
        &mut self,
        provider: Arc<Provider<P>>,
    ) -> Result<(), PairSyncError<P>> {
        self.token_a_decimals = abi::IErc20::new(self.token_a, provider.clone())
            .decimals()
            .call()
            .await?;

        self.token_b_decimals = abi::IErc20::new(self.token_a, provider)
            .decimals()
            .call()
            .await?;

        Ok(())
    }
}
