use ethers::{
    providers::{Http, Middleware, Provider},
    types::{Address, BlockId, Bytes, Chain, TxHash},
};
use futures::future;
use std::{collections::HashMap, env, fs, path::PathBuf, str::FromStr, sync::Arc};

pub struct ContractCreation {
    tx_hash: TxHash,
    block: BlockId,
    creation_code: Bytes,
}

pub fn provider_from_chain(chain: Chain) -> Arc<Provider<Http>> {
    match chain {
        Chain::XDai => {
            Arc::new(Provider::<Http>::try_from(env::var("GNOSIS_CHAIN_RPC_URL").unwrap()).unwrap())
        }
        Chain::Goerli => {
            Arc::new(Provider::<Http>::try_from(env::var("GOERLI_RPC_URL").unwrap()).unwrap())
        }
        Chain::Mainnet => {
            Arc::new(Provider::<Http>::try_from(env::var("MAINNET_RPC_URL").unwrap()).unwrap())
        }
        Chain::Optimism => {
            Arc::new(Provider::<Http>::try_from(env::var("OPTIMISM_RPC_URL").unwrap()).unwrap())
        }
        Chain::Polygon => {
            Arc::new(Provider::<Http>::try_from(env::var("POLYGON_RPC_URL").unwrap()).unwrap())
        }
        _ => panic!("Unsupported chain"),
    }
}

pub fn provider_url_from_chain(chain: Chain) -> String {
    match chain {
        Chain::XDai => env::var("GNOSIS_CHAIN_RPC_URL").unwrap(),
        Chain::Goerli => env::var("GOERLI_RPC_URL").unwrap(),
        Chain::Mainnet => env::var("MAINNET_RPC_URL").unwrap(),
        Chain::Optimism => env::var("OPTIMISM_RPC_URL").unwrap(),
        Chain::Polygon => env::var("POLYGON_RPC_URL").unwrap(),
        _ => panic!("Unsupported chain"),
    }
}

pub async fn contract_creation_data(
    provider: &Arc<Provider<Http>>,
    tx_hash: TxHash,
) -> Option<Bytes> {
    provider
        .get_transaction(tx_hash)
        .await
        .ok()
        .and_then(|tx| tx.map(|tx| if tx.to.is_none() { Some(tx.input) } else { None }))
        .flatten()
}

pub async fn contract_runtime_code(provider: &Arc<Provider<Http>>, address: Address) -> Bytes {
    provider.get_code(address, None).await.unwrap()
}

#[derive(Debug)]
pub struct ChainResponse<T> {
    responses: HashMap<Chain, Option<T>>,
}

impl<T> ChainResponse<T> {
    pub fn is_all_none(&self) -> bool {
        self.responses.values().all(|value| value.is_none())
    }

    pub fn iter_entries(&self) -> impl Iterator<Item = (&Chain, &T)> {
        self.responses.iter().filter_map(|(key, value)| value.as_ref().map(|v| (key, v)))
    }
}

pub struct MultiChainProvider {
    providers: HashMap<Chain, Arc<Provider<Http>>>,
}

impl Default for MultiChainProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiChainProvider {
    pub fn new() -> Self {
        let params = [
            (Chain::XDai, "GNOSIS_CHAIN_RPC_URL"),
            (Chain::Goerli, "GOERLI_RPC_URL"),
            (Chain::Mainnet, "MAINNET_RPC_URL"),
            (Chain::Optimism, "OPTIMISM_RPC_URL"),
            (Chain::Polygon, "POLYGON_RPC_URL"),
        ];

        let providers = params
            .iter()
            .map(|(chain, env_var)| {
                (*chain, Arc::new(Provider::<Http>::try_from(env::var(*env_var).unwrap()).unwrap()))
            })
            .collect::<HashMap<_, _>>();

        Self { providers }
    }

    pub async fn get_creation_code(&self, address: Address) -> ChainResponse<ContractCreation> {
        async fn find_creation_code(
            provider: &Arc<Provider<Http>>,
            address: Address,
        ) -> Option<ContractCreation> {
            let creation_block = find_creation_block(provider, address).await;
            if creation_block.is_err() {
                return None
            }
            let creation_tx = find_creation_tx(provider, address, creation_block.unwrap()).await;
            if creation_tx.is_err() {
                None
            } else {
                Some(creation_tx.unwrap())
            }
        }

        let futures = self.providers.iter().map(|(chain, provider)| async move {
            (*chain, find_creation_code(provider, address).await)
        });
        let responses = future::join_all(futures).await.into_iter().collect::<HashMap<_, _>>();
        ChainResponse { responses }
    }

    pub fn compare_creation_code(
        &self,
        artifacts: Vec<PathBuf>,
        creation_data: &ChainResponse<ContractCreation>,
    ) -> ChainResponse<PathBuf> {
        fn compare(
            artifacts: Vec<PathBuf>,
            expected_creation_data: &ContractCreation,
        ) -> Option<PathBuf> {
            for artifact in artifacts {
                let content = fs::read_to_string(&artifact).unwrap();
                let json: serde_json::Value = serde_json::from_str(&content).unwrap();
                if let Some(bytecode_value) = json.get("bytecode").unwrap().get("object") {
                    if let Some(bytecode_str) = bytecode_value.as_str() {
                        let bytecode = Bytes::from_str(bytecode_str).unwrap();
                        // TODO This check won't always work, e.g. constructor args, metadata hash,
                        // etc.
                        if bytecode == expected_creation_data.creation_code {
                            return Some(artifact)
                        }
                    }
                }
            }
            None
        }

        let responses = self
            .providers
            .keys()
            .map(|chain| {
                let artifacts = artifacts.clone();
                let expected_creation_data = creation_data.responses.get(chain).unwrap();
                if expected_creation_data.is_none() {
                    return (*chain, None)
                }
                let expected_creation_data = expected_creation_data.as_ref().unwrap();
                (*chain, compare(artifacts, expected_creation_data))
            })
            .collect::<HashMap<_, _>>();

        ChainResponse { responses }
    }
}

async fn find_creation_block(
    provider: &Arc<Provider<Http>>,
    address: Address,
) -> Result<BlockId, Box<dyn std::error::Error + Send + Sync>> {
    let latest_block_num = provider.get_block_number().await?.as_u64();
    let latest_block = BlockId::from(latest_block_num);
    let has_code = !provider.get_code(address, Some(latest_block)).await?.is_empty();
    if !has_code {
        return Err("Contract does not exist".into())
    }

    // Binary search to find the block where the contract was created.
    // TODO Consider biasing this towards recent blocks to reduce RPC requests. Currently the max
    // number of RPC requests used is log2(num_blocks). For 17M mainnet blocks this is 24 RPC calls.
    let mut low = 0;
    let mut high = latest_block_num;
    while low < high {
        let mid = (low + high) / 2;
        let block = BlockId::from(mid);
        let has_code = !provider.get_code(address, Some(block)).await.unwrap().is_empty();
        if has_code {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    Ok(BlockId::from(high))
}

async fn find_creation_tx(
    provider: &Arc<Provider<Http>>,
    address: Address,
    block: BlockId,
) -> Result<ContractCreation, Box<dyn std::error::Error + Send + Sync>> {
    let block_data = provider.get_block(block).await?.ok_or("Block not found")?;

    for tx_hash in block_data.transactions {
        let tx = provider.get_transaction(tx_hash).await?.ok_or("Transaction not found")?;

        // Regular CREATE transaction.
        if tx.to.is_none() {
            // TODO Compute the expected CREATE address to save an RPC call.
            let receipt =
                provider.get_transaction_receipt(tx_hash).await?.ok_or("Receipt not found")?;
            if let Some(contract_address) = receipt.contract_address {
                if contract_address == address {
                    let creation_code = tx.input;
                    return Ok(ContractCreation { tx_hash, block, creation_code })
                }
            }
        }

        // Contract was deployed from a factory. For now, to avoid tracing, we hardcode a few known,
        // popular create2 factories.
        if let Some(factory) = tx.to {
            // https://github.com/Arachnid/deterministic-deployment-proxy
            if factory == Address::from_str("0x4e59b44847b379578588920cA78FbF26c0B4956C")? {
                // TODO
            }
            // Create2 factory by 0age.
            if factory == Address::from_str("0x0000000000FFe8B47B3e2130213B802212439497")? {
                // TODO
            }
        }
    }
    Err("Contract creation transaction not found. It may have been deployed by an unsupported factory.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::try_join_all;

    fn get_provider() -> Arc<Provider<Http>> {
        Arc::new(Provider::<Http>::try_from(env::var("GOERLI_RPC_URL").unwrap()).unwrap())
    }

    #[tokio::test]
    async fn test_find_creation_block() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let provider = get_provider();

        // Define contract addresses with their corresponding creation blocks.
        let test_cases = vec![
            ("0xc9E7278C9f386f307524eBbAaafcfEb649Be39b4", BlockId::from(8666991)), /* Counter. */
            ("0x1F98431c8aD98523631AE4a59f267346ea31F984", BlockId::from(4734394)), /* UniswapV3Factory. */
            ("0x00000000000001ad428e4906aE43D8F9852d0dD6", BlockId::from(8515378)), /* Seaport. */
        ];

        let tasks = test_cases.into_iter().map(|(contract, expected_block)| {
            let provider = provider.clone();
            async move {
                let contract_addr = Address::from_str(contract)?;
                let creation_block = find_creation_block(&provider, contract_addr).await?;
                assert_eq!(creation_block, expected_block);
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            }
        });

        try_join_all(tasks).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_find_creation_tx() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let provider = get_provider();

        #[rustfmt::skip]
        let test_cases = vec![
            // Counter, CREATE.
            ("0xc9E7278C9f386f307524eBbAaafcfEb649Be39b4", "0x005c7b8f0ccbd49ff8892ec0ef27058b79d9a1ed6592faaa44699cccce1aa350"),
            // UniswapV3Factory, CREATE.
            ("0x1F98431c8aD98523631AE4a59f267346ea31F984", "0x7f0c3a53db387e9b3ff4af69c2ae9c45182ba189b2c1d3607e6a5e1cdab29fc8"),
            // Seaport, CREATE2, not yet supported.
            // ("0x00000000000001ad428e4906aE43D8F9852d0dD6", "0x48ad9bd93b31a55c08cfd99b48bea139e9f448f0bff1ab03d064ae6dce09f7f6"),
        ];

        let tasks = test_cases.into_iter().map(|(contract, tx_hash)| {
            let provider = provider.clone();
            async move {
                let contract_addr = Address::from_str(contract)?;
                let expected_tx_hash = TxHash::from_str(tx_hash)?;
                let creation_block = find_creation_block(&provider, contract_addr).await?;
                let creation_tx =
                    find_creation_tx(&provider, contract_addr, creation_block).await?;
                assert_eq!(creation_tx.tx_hash, expected_tx_hash);
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            }
        });

        try_join_all(tasks).await?;
        Ok(())
    }
}
