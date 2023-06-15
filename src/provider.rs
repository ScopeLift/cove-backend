use crate::{
    bytecode::{creation_code_equality_check, deployed_code_equality_check, MatchType},
    frameworks::Framework,
};
use ethers::{
    providers::{Http, Middleware, Provider},
    types::{Address, BlockNumber, Bytes, Chain, Transaction, TxHash, U256},
};
use futures::future;
use std::{collections::HashMap, env, error::Error, path::PathBuf, str::FromStr, sync::Arc};

pub struct ContractCreation {
    pub tx_hash: TxHash,
    pub block: BlockNumber,
    pub creation_code: Bytes,
}

#[derive(Debug, Default, Clone)]
pub struct ContractMatch {
    pub artifact: PathBuf,
    pub match_type: MatchType,
}

// ==============================
// ======== Single Chain ========
// ==============================

pub fn provider_from_chain(chain: Chain) -> Arc<Provider<Http>> {
    match chain {
        // Mainnet + Testnets.
        Chain::Mainnet => {
            println!("MAINNET_RPC_URL {}", env::var("MAINNET_RPC_URL").unwrap());
            Arc::new(Provider::<Http>::try_from(env::var("MAINNET_RPC_URL").unwrap()).unwrap())
        }
        Chain::Goerli => {
            println!("GOERLI_RPC_URL {}", env::var("GOERLI_RPC_URL").unwrap());
            Arc::new(Provider::<Http>::try_from(env::var("GOERLI_RPC_URL").unwrap()).unwrap())
        }
        Chain::Sepolia => {
            println!("SEPOLIA_RPC_URL {}", env::var("SEPOLIA_RPC_URL").unwrap());
            Arc::new(Provider::<Http>::try_from(env::var("SEPOLIA_RPC_URL").unwrap()).unwrap())
        }
        // Other chains.
        Chain::Optimism => {
            println!("OPTIMISM_RPC_URL {}", env::var("OPTIMISM_RPC_URL").unwrap());
            Arc::new(Provider::<Http>::try_from(env::var("OPTIMISM_RPC_URL").unwrap()).unwrap())
        }
        Chain::Arbitrum => {
            println!("ARBITRUM_ONE_RPC_URL {}", env::var("ARBITRUM_ONE_RPC_URL").unwrap());
            Arc::new(Provider::<Http>::try_from(env::var("ARBITRUM_ONE_RPC_URL").unwrap()).unwrap())
        }
        Chain::Polygon => {
            println!("POLYGON_RPC_URL {}", env::var("POLYGON_RPC_URL").unwrap());
            Arc::new(Provider::<Http>::try_from(env::var("POLYGON_RPC_URL").unwrap()).unwrap())
        }
        Chain::XDai => {
            println!("GNOSIS_CHAIN_RPC_URL {}", env::var("GNOSIS_CHAIN_RPC_URL").unwrap());
            Arc::new(Provider::<Http>::try_from(env::var("GNOSIS_CHAIN_RPC_URL").unwrap()).unwrap())
        }
        _ => panic!("Unsupported chain"),
    }
}

pub fn provider_url_from_chain(chain: Chain) -> String {
    match chain {
        // Mainnet + Testnets.
        Chain::Mainnet => env::var("MAINNET_RPC_URL").unwrap(),
        Chain::Goerli => env::var("GOERLI_RPC_URL").unwrap(),
        Chain::Sepolia => env::var("SEPOLIA_RPC_URL").unwrap(),
        // Other chains.
        Chain::Optimism => env::var("OPTIMISM_RPC_URL").unwrap(),
        Chain::Arbitrum => env::var("ARBITRUM_ONE_RPC_URL").unwrap(),
        Chain::Polygon => env::var("POLYGON_RPC_URL").unwrap(),
        Chain::XDai => env::var("GNOSIS_CHAIN_RPC_URL").unwrap(),
        _ => panic!("Unsupported chain"),
    }
}

pub async fn contract_runtime_code(provider: &Arc<Provider<Http>>, address: Address) -> Bytes {
    provider.get_code(address, None).await.unwrap()
}

// =============================
// ======== Multi-Chain ========
// =============================

#[derive(Debug, Default)]
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
    pub chains: Vec<Chain>,
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
            vec![Chain::XDai, Chain::Goerli, Chain::Mainnet, Chain::Optimism, Chain::Polygon];

        let providers = chains
            .iter()
            .map(|chain| (*chain, provider_from_chain(*chain)))
            .collect::<HashMap<_, _>>();

        Self { chains, providers }
    }

    pub async fn get_creation_code(
        &self,
        address: Address,
        creation_tx_hashes: Option<HashMap<Chain, TxHash>>,
    ) -> Result<ChainResponse<ContractCreation>, Box<dyn Error + Send + Sync>> {
        async fn find_creation_code(
            provider: &Arc<Provider<Http>>,
            address: Address,
            creation_tx_hash: Option<TxHash>,
        ) -> Option<ContractCreation> {
            let creation_data =
                find_creation_data(provider, address, creation_tx_hash).await.ok()?;
            Some(creation_data)
        }

        let futures = self.providers.iter().map(|(chain, provider)| {
            let creation_tx_hash = creation_tx_hashes.as_ref().and_then(|h| h.get(chain)).cloned();
            async move { (*chain, find_creation_code(provider, address, creation_tx_hash).await) }
        });
        let responses = future::join_all(futures).await.into_iter().collect::<HashMap<_, _>>();
        Ok(ChainResponse { responses })
    }

    pub async fn get_deployed_code(
        &self,
        address: Address,
    ) -> Result<ChainResponse<Bytes>, Box<dyn Error>> {
        async fn find_deployed_code(
            provider: &Arc<Provider<Http>>,
            address: Address,
        ) -> Option<Bytes> {
            println!("provider {:?}", provider);
            let code = provider.get_code(address, None).await.ok()?;
            println!("code {:?}", code);
            if code.is_empty() {
                None
            } else {
                Some(code)
            }
        }

        let futures = self.providers.iter().map(|(chain, provider)| async move {
            (*chain, find_deployed_code(provider, address).await)
        });
        let responses = future::join_all(futures).await.into_iter().collect::<HashMap<_, _>>();
        println!("responses {:?}", responses);
        Ok(ChainResponse { responses })
    }

    pub fn compare_creation_code(
        &self,
        project: &impl Framework,
        creation_data: &ChainResponse<ContractCreation>,
    ) -> ChainResponse<ContractMatch> {
        fn compare(
            project: &impl Framework,
            expected_creation_code: &Bytes,
        ) -> Option<ContractMatch> {
            let artifacts = project.get_artifacts().unwrap();
            if artifacts.is_empty() {
                panic!("No artifacts found in project");
            }

            let mut best_artifact_match: Option<ContractMatch> = None;
            for artifact in artifacts {
                println!("Checking creation artifact: {:?}", artifact.as_path());
                let found = match project.structure_found_creation_code(&artifact) {
                    Ok(found) => found,
                    Err(_) => continue,
                };

                let expected = match project.structure_expected_creation_code(
                    &artifact,
                    &found,
                    expected_creation_code,
                ) {
                    Ok(expected) => expected,
                    Err(_) => continue,
                };

                // If we have an exact match, return it. If we have a partial match, save it off.
                // We'll return it if we don't find an exact match. Note that treats all partial
                // matches equally and arbitrarily gives priority to the last one.
                match creation_code_equality_check(&found, &expected) {
                    MatchType::Full => {
                        return Some(ContractMatch { artifact, match_type: MatchType::Full })
                    }
                    MatchType::Partial => {
                        best_artifact_match =
                            Some(ContractMatch { artifact, match_type: MatchType::Partial })
                    }
                    _ => {}
                }
            }
            best_artifact_match
        }

        let responses = self
            .providers
            .keys()
            .map(|chain| {
                let expected_creation_data = creation_data.responses.get(chain).unwrap();
                if expected_creation_data.is_none() {
                    return (*chain, None)
                }
                let expected_creation_code =
                    &expected_creation_data.as_ref().unwrap().creation_code;
                (*chain, compare(project, expected_creation_code))
            })
            .collect::<HashMap<_, _>>();

        ChainResponse { responses }
    }

    pub fn compare_deployed_code(
        &self,
        project: &impl Framework,
        deployed_code: &ChainResponse<Bytes>,
    ) -> ChainResponse<ContractMatch> {
        fn compare(
            project: &impl Framework,
            expected_deployed_code: &Bytes,
        ) -> Option<ContractMatch> {
            let artifacts = project.get_artifacts().unwrap();
            if artifacts.is_empty() {
                panic!("No artifacts found in project");
            }

            let mut best_artifact_match: Option<ContractMatch> = None;
            for artifact in artifacts {
                let found = match project.structure_found_deployed_code(&artifact) {
                    Ok(found) => found,
                    Err(_) => continue,
                };

                let expected = match project
                    .structure_expected_deployed_code(&found, expected_deployed_code)
                {
                    Ok(expected) => expected,
                    Err(_) => continue,
                };

                // If we have an exact match, return it. If we have a partial match, save it off.
                // We'll return it if we don't find an exact match. Note that treats all partial
                // matches equally and arbitrarily gives priority to the last one.
                match deployed_code_equality_check(&found, &expected) {
                    MatchType::Full => {
                        return Some(ContractMatch { artifact, match_type: MatchType::Full })
                    }
                    MatchType::Partial => {
                        best_artifact_match =
                            Some(ContractMatch { artifact, match_type: MatchType::Partial })
                    }
                    _ => {}
                }
            }
            best_artifact_match
        }

        let responses = self
            .providers
            .keys()
            .map(|chain| {
                let expected_deployed_code = deployed_code.responses.get(chain).unwrap();
                if expected_deployed_code.is_none() {
                    return (*chain, None)
                }
                let expected_creation_code = &expected_deployed_code.as_ref().unwrap();
                (*chain, compare(project, expected_creation_code))
            })
            .collect::<HashMap<_, _>>();

        ChainResponse { responses }
    }
}

async fn find_creation_data(
    provider: &Arc<Provider<Http>>,
    address: Address,
    tx_hash: Option<TxHash>,
) -> Result<ContractCreation, Box<dyn std::error::Error + Send + Sync>> {
    // If we have a transaction hash, use that to find the creation code.
    if let Some(tx_hash) = tx_hash {
        let (creation_code, tx) = creation_code_from_tx_hash(provider, address, tx_hash).await?;
        let block = BlockNumber::from(tx.block_number.unwrap());
        return Ok(ContractCreation { tx_hash, block, creation_code })
    }

    Err("Automatically finding creation data is currently not supported.".into())
}

async fn creation_code_from_tx_hash(
    provider: &Arc<Provider<Http>>,
    address: Address,
    tx_hash: TxHash,
) -> Result<(Bytes, Transaction), Box<dyn std::error::Error + Send + Sync>> {
    let tx = provider.get_transaction(tx_hash).await?.ok_or("Transaction not found")?;

    // Regular CREATE transaction.
    if tx.to.is_none() {
        let receipt =
            provider.get_transaction_receipt(tx_hash).await?.ok_or("Receipt not found")?;
        if let Some(contract_address) = receipt.contract_address {
            if contract_address == address {
                let creation_code = tx.input.clone();
                return Ok((creation_code, tx))
            }
        }
    }

    // Contract was deployed from a factory. For now, to avoid tracing, we hardcode a few known,
    // popular create2 factories.
    if let Some(factory) = tx.to {
        // https://github.com/Arachnid/deterministic-deployment-proxy
        if factory == Address::from_str("0x4e59b44847b379578588920cA78FbF26c0B4956C")? {
            // The first 32 bytes of calldata are the salt, and the rest are the creation code.
            let creation_code = Bytes::from_iter(tx.input[32..].to_vec());
            return Ok((creation_code, tx))
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
            let len = U256::from(len).as_usize();
            let creation_code = &tx.input[100..len + 100];
            let creation_code = Bytes::from_iter(creation_code);
            return Ok((creation_code, tx))
        }
    }
    Err("Contract creation transaction not found. It may have been deployed by an unsupported factory, or the wrong transaction hash for this chain was provided.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotenvy::dotenv;
    use futures::future::try_join_all;

    fn get_provider() -> Arc<Provider<Http>> {
        if dotenv().is_err() {
            // We don't error since there's no `.env` file in CI.
            println!("WARNING: No .env file found, using default environment variables.");
        }
        Arc::new(Provider::<Http>::try_from(env::var("GOERLI_RPC_URL").unwrap()).unwrap())
    }

    #[tokio::test]
    async fn test_find_creation_data() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
                let creation_data =
                    find_creation_data(&provider, contract_addr, Some(expected_tx_hash)).await?;
                assert_eq!(creation_data.tx_hash, expected_tx_hash, "{name}");
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            }
        });

        try_join_all(tasks).await?;
        Ok(())
    }
}
