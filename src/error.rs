use ethers::prelude::ContractError;
use ethers::providers::{JsonRpcClient, Provider, ProviderError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PairSyncError<P>
where
    P: JsonRpcClient,
{
    #[error("Provider error")]
    ProviderError(#[from] ProviderError),
    #[error("Contract error")]
    ContractError(#[from] ContractError<Provider<P>>),
}
