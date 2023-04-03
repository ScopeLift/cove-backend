use colored::Colorize;
use ethers::{
    providers::{Http, Middleware, Provider},
    types::{Address, BlockId, BlockNumber, Bytes, Chain, TxHash, U256},
};
use futures::future;
use std::{collections::HashMap, env, fs, path::PathBuf, str::FromStr, sync::Arc};

pub struct ContractCreation {
    pub tx_hash: TxHash,
    pub block: BlockNumber,
    pub creation_code: Bytes,
}

fn print_color_by_chain(text: String, chain: Chain) {
    match chain {
        Chain::Goerli => println!("{}", text.yellow()),
        Chain::Mainnet => println!("{}", text.blue()),
        Chain::Optimism => println!("{}", text.red()),
        Chain::Polygon => println!("{}", text.purple()),
        _ => println!("{}", text),
    }
}

// ==============================
// ======== Single Chain ========
// ==============================

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

pub async fn contract_runtime_code(provider: &Arc<Provider<Http>>, address: Address) -> Bytes {
    provider.get_code(address, None).await.unwrap()
}

// =============================
// ======== Multi-Chain ========
// =============================

#[derive(Debug)]
pub struct ChainResponse<T> {
    pub responses: HashMap<Chain, Option<T>>,
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
    pub providers: HashMap<Chain, Arc<Provider<Http>>>,
}

impl Default for MultiChainProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiChainProvider {
    pub fn new() -> Self {
        let chains =
            // Gnosis Chain fails to binary search with: `No state available for block 13600864`. Need to
            // get access an archive node to get the binary search working.
            // vec![Chain::XDai, Chain::Goerli, Chain::Mainnet, Chain::Optimism, Chain::Polygon];
            vec![Chain::Goerli, Chain::Mainnet, Chain::Optimism, Chain::Polygon];

        let providers = chains
            .iter()
            .map(|chain| (*chain, provider_from_chain(*chain)))
            .collect::<HashMap<_, _>>();

        Self { providers }
    }

    pub async fn get_creation_code(&self, address: Address) -> ChainResponse<ContractCreation> {
        async fn find_creation_code(
            provider: &Arc<Provider<Http>>,
            address: Address,
        ) -> Option<ContractCreation> {
            let creation_block = find_creation_block(provider, address).await.ok()?;
            let creation_tx = find_creation_tx(provider, address, creation_block).await.ok()?;
            Some(creation_tx)
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

                        // First just try a simple equality check.
                        if bytecode == expected_creation_data.creation_code {
                            return Some(artifact)
                        }

                        // Let's try accounting for constructor arguments now. We do this by
                        // defining equality as:
                        //   - The expected creation code is longer than the artifact creation code.
                        //   - The expected creation code and found creation code match up to byte
                        //     `n`, where `n` is the length of the found bytecode.
                        let equal_len = expected_creation_data.creation_code.len() > bytecode.len();
                        let bytes_match = bytecode
                            .iter()
                            .zip(expected_creation_data.creation_code.iter())
                            .all(|(a_byte, b_byte)| a_byte == b_byte);

                        if equal_len && bytes_match {
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
) -> Result<BlockNumber, Box<dyn std::error::Error + Send + Sync>> {
    let chain_id = provider.get_chainid().await?;
    let chain_name = Chain::try_from(chain_id)?;
    print_color_by_chain(
        format!("  {:?}: Checking if there is code at this address.", chain_name),
        chain_name,
    );
    let latest_block_num = provider.get_block_number().await?.as_u64();
    let latest_block = BlockId::from(latest_block_num);
    let has_code = !provider.get_code(address, Some(latest_block)).await?.is_empty();
    if !has_code {
        print_color_by_chain(format!("  {:?}: No code, returning.", chain_name), chain_name);
        return Err("Contract does not exist".into())
    }

    // Binary search to find the block where the contract was created.
    // TODO Consider biasing this towards recent blocks to reduce RPC requests. Currently the max
    // number of RPC requests used is log2(num_blocks). For 17M mainnet blocks this is 24 RPC calls.
    print_color_by_chain(
        format!("  {:?}: Binary searching over all blocks to find deployment block.", chain_name),
        chain_name,
    );
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

    print_color_by_chain(
        format!("  {:?}: Found deployment block {:?}.", chain_name, high),
        chain_name,
    );
    Ok(BlockNumber::from(high))
}

async fn find_creation_tx(
    provider: &Arc<Provider<Http>>,
    address: Address,
    block: BlockNumber,
) -> Result<ContractCreation, Box<dyn std::error::Error + Send + Sync>> {
    let chain_id = provider.get_chainid().await?;
    let chain_name = Chain::try_from(chain_id)?;
    print_color_by_chain(
        format!("  {:?}: Finding deployment transaction and creation code.", chain_name),
        chain_name,
    );
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
                    print_color_by_chain(
                        format!("  {:?}: Found transaction hash {:?}.", chain_name, tx_hash),
                        chain_name,
                    );
                    return Ok(ContractCreation { tx_hash, block, creation_code })
                }
            }
        }

        // Contract was deployed from a factory. For now, to avoid tracing, we hardcode a few known,
        // popular create2 factories.
        // TODO Currently assumes the first found transaction hash is the right one.
        if let Some(factory) = tx.to {
            // https://github.com/Arachnid/deterministic-deployment-proxy
            if factory == Address::from_str("0x4e59b44847b379578588920cA78FbF26c0B4956C")? {
                todo!();
            }
            // Create2 factory by 0age.
            if factory == Address::from_str("0x0000000000FFe8B47B3e2130213B802212439497")? {
                // The only function on this deployer is:
                //   `function safeCreate2(bytes32 salt, bytes calldata initializationCode)`
                // so we know that method was called and can extract the creation code. The input
                // data is structured as follows:
                //   - Bytes 1-4: Function selector
                //   - Bytes 5-36: Salt
                //   - Bytes 37-68: Offset to creation code data
                //   - Bytes 69-100: Offset to creation code length
                let len = &tx.input[69..100];
                let len = U256::from(len).as_usize() + 100; // TODO Why, without this, is the code 100 bytes short?
                let creation_code = &tx.input[100..len];
                let creation_code = Bytes::from_iter(creation_code);

                print_color_by_chain(
                    format!("  {:?}: Found transaction hash {:?}.", chain_name, tx_hash),
                    chain_name,
                );
                return Ok(ContractCreation { tx_hash, block, creation_code })
            }
        }
    }
    Err("Contract creation transaction not found. It may have been deployed by an unsupported factory.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotenvy::dotenv;
    use futures::future::try_join_all;

    fn get_provider() -> Arc<Provider<Http>> {
        dotenv().expect(".env file not found");
        Arc::new(Provider::<Http>::try_from(env::var("GOERLI_RPC_URL").unwrap()).unwrap())
    }

    #[tokio::test]
    async fn test_find_creation_block() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let provider = get_provider();

        // Define contract addresses with their corresponding creation blocks.
        #[rustfmt::skip]
        let test_cases = vec![
            ("0xc9E7278C9f386f307524eBbAaafcfEb649Be39b4", BlockNumber::from(8666991), "Counter"),
            ("0x1F98431c8aD98523631AE4a59f267346ea31F984", BlockNumber::from(4734394), "UniV3Factory"),
            ("0x00000000000001ad428e4906aE43D8F9852d0dD6", BlockNumber::from(8515378), "Seaport"),
        ];

        let tasks = test_cases.into_iter().map(|(contract, expected_block, name)| {
            let provider = provider.clone();
            async move {
                let contract_addr = Address::from_str(contract)?;
                let creation_block = find_creation_block(&provider, contract_addr).await?;
                assert_eq!(creation_block, expected_block, "{name}");
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
            ("0xc9E7278C9f386f307524eBbAaafcfEb649Be39b4", "0x005c7b8f0ccbd49ff8892ec0ef27058b79d9a1ed6592faaa44699cccce1aa350", "Counter, CREATE"),
            ("0x1F98431c8aD98523631AE4a59f267346ea31F984", "0x7f0c3a53db387e9b3ff4af69c2ae9c45182ba189b2c1d3607e6a5e1cdab29fc8", "UniV3Factory, CREATE"),
            ("0x00000000000001ad428e4906aE43D8F9852d0dD6", "0x48ad9bd93b31a55c08cfd99b48bea139e9f448f0bff1ab03d064ae6dce09f7f6", "Seaport, CREATE2"),
        ];

        let tasks = test_cases.into_iter().map(|(contract, tx_hash, name)| {
            let provider = provider.clone();
            async move {
                let contract_addr = Address::from_str(contract)?;
                let expected_tx_hash = TxHash::from_str(tx_hash)?;
                let creation_block = find_creation_block(&provider, contract_addr).await?;
                let creation_tx =
                    find_creation_tx(&provider, contract_addr, creation_block).await?;
                assert_eq!(creation_tx.tx_hash, expected_tx_hash, "{name}");
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            }
        });

        try_join_all(tasks).await?;
        Ok(())
    }
}
