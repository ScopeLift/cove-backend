use crate::{
    bytecode::{creation_code_equality_check, deployed_code_equality_check, MatchType},
    frameworks::framework::Framework,
};
use ethers::{
    providers::{Http, Middleware, Provider},
    types::{Address, BlockNumber, Bytes, Chain, Transaction, TxHash, U256},
};
use futures::future;
use std::{collections::HashMap, env, error::Error, path::PathBuf, str::FromStr, sync::Arc};

/// Contract creation data.
pub struct ContractCreation {
    /// The transaction hash of the contract creation transaction.
    pub tx_hash: TxHash,
    /// The block number of the contract creation transaction.
    pub block: BlockNumber,
    /// The creation code of the contract.
    pub creation_code: Bytes,
}

/// Match data for a given artifact.
#[derive(Debug, Default, Clone)]
pub struct ContractMatch {
    /// Path to the artifact.
    pub artifact: PathBuf,
    /// The type of match for that artifact against the expected code.
    pub match_type: MatchType,
}

// ==============================
// ======== Single Chain ========
// ==============================

/// Create a provider for the given chain.
pub fn provider_from_chain(chain: Chain) -> Arc<Provider<Http>> {
    match chain {
        // Mainnet + Testnets.
        Chain::Mainnet => {
            Arc::new(Provider::<Http>::try_from(provider_url_from_chain(chain)).unwrap())
        }
        Chain::Goerli => {
            Arc::new(Provider::<Http>::try_from(provider_url_from_chain(chain)).unwrap())
        }
        Chain::Sepolia => {
            Arc::new(Provider::<Http>::try_from(provider_url_from_chain(chain)).unwrap())
        }
        // Other chains.
        Chain::Optimism => {
            Arc::new(Provider::<Http>::try_from(provider_url_from_chain(chain)).unwrap())
        }
        Chain::Arbitrum => {
            Arc::new(Provider::<Http>::try_from(provider_url_from_chain(chain)).unwrap())
        }
        Chain::Polygon => {
            Arc::new(Provider::<Http>::try_from(provider_url_from_chain(chain)).unwrap())
        }
        Chain::XDai => {
            Arc::new(Provider::<Http>::try_from(provider_url_from_chain(chain)).unwrap())
        }
        Chain::Avalanche => {
            Arc::new(Provider::<Http>::try_from(provider_url_from_chain(chain)).unwrap())
        }
        _ => panic!("Unsupported chain"),
    }
}

/// Return the RPC provider URL for the given chain.
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
        Chain::Avalanche => env::var("AVALANCHE_RPC_URL").unwrap(),
        _ => panic!("Unsupported chain"),
    }
}

/// Return the runtime code at the given address using the given provider.
pub async fn contract_runtime_code(provider: &Arc<Provider<Http>>, address: Address) -> Bytes {
    provider.get_code(address, None).await.unwrap()
}

// =============================
// ======== Multi-Chain ========
// =============================

/// The response from a multi-chain provider's query.
#[derive(Debug, Default)]
pub struct ChainResponse<T> {
    /// A mapping from chain to the response for that chain.
    pub responses: HashMap<Chain, Option<T>>,
}

impl<T> ChainResponse<T> {
    /// Returns `true` if all responses are `None`, `false` otherwise.
    pub fn is_all_none(&self) -> bool {
        self.responses.values().all(|value| value.is_none())
    }

    /// Returns an iterator over the `Some` entries of the response.
    pub fn iter_entries(&self) -> impl Iterator<Item = (&Chain, &T)> {
        self.responses.iter().filter_map(|(key, value)| value.as_ref().map(|v| (key, v)))
    }
}

/// A provider that performs the same queries or operations across multiple chains simultaneously.
pub struct MultiChainProvider {
    /// The chains that this provider supports.
    pub chains: Vec<Chain>,
    /// The provider for each chain.
    pub providers: HashMap<Chain, Arc<Provider<Http>>>,
}

impl Default for MultiChainProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiChainProvider {
    /// Create a new `MultiChainProvider` with all supported chains.
    pub fn new() -> Self {
        let chains = vec![
            Chain::Arbitrum,
            Chain::Goerli,
            Chain::Mainnet,
            Chain::Optimism,
            Chain::Polygon,
            Chain::Sepolia,
            Chain::XDai,
            Chain::Avalanche,
        ];

        let providers = chains
            .iter()
            .map(|chain| (*chain, provider_from_chain(*chain)))
            .collect::<HashMap<_, _>>();

        Self { chains, providers }
    }

    /// Given an address, return the creation code at that address for each supported chain.
    pub async fn get_creation_code(
        &self,
        address: Address,
        creation_tx_hashes: Option<HashMap<Chain, TxHash>>,
    ) -> Result<ChainResponse<ContractCreation>, Box<dyn Error + Send + Sync>> {
        /// Given an address, return the creation code at that address for the chain specified by
        /// the provider.
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

    /// Given an address, return the deployed code at that address for each supported chain.
    pub async fn get_deployed_code(
        &self,
        address: Address,
    ) -> Result<ChainResponse<Bytes>, Box<dyn Error>> {
        /// Given an address, return the deployed code at that address for the chain specified by
        /// the given provider.
        async fn find_deployed_code(
            provider: &Arc<Provider<Http>>,
            address: Address,
        ) -> Option<Bytes> {
            let code = provider.get_code(address, None).await.ok()?;
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
        Ok(ChainResponse { responses })
    }

    /// Given the creation code data being compared against and the build artifacts from a project,
    /// compare the creation code against the expected creation code for each artifact and return
    /// the best match found. It's possible that no match is found.
    pub fn compare_creation_code(
        &self,
        project: &impl Framework,
        creation_data: &ChainResponse<ContractCreation>,
    ) -> ChainResponse<ContractMatch> {
        /// Compares the creation code against the expected creation code for each artifact and
        /// returns the best match.
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

    /// Given the deployed code being compared against and the build artifacts from a project,
    /// compare the deployed code against the expected deployed code for each artifact and return
    /// the best match found. It's possible that no match is found.
    pub fn compare_deployed_code(
        &self,
        project: &impl Framework,
        deployed_code: &ChainResponse<Bytes>,
    ) -> ChainResponse<ContractMatch> {
        /// Compares the deployed code against the expected deployed code for each artifact and
        /// returns the best match.
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

/// Given the transaction hash of a contract creation transaction, extracts the creation code from
/// the transaction and returns the creation data. This feature is currently not supported.
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

/// Given the transaction hash of a contract creation transaction, extracts the creation code from
/// the transaction. This feature is currently not supported.
async fn creation_code_from_tx_hash(
    provider: &Arc<Provider<Http>>,
    address: Address,
    tx_hash: TxHash,
) -> Result<(Bytes, Transaction), Box<dyn std::error::Error + Send + Sync>> {
    // TODO This is not currently supported, but the flow would be as follows:
    //   1. Fetch the transaction data.
    //   2. If `to` is None, this was a regular CREATE transaction so we can extract the creation
    //      code from the input data.
    //   3. Otherwise, the contract was deployed by a factory. First, check the `to` address and see
    //      if it's a known factory. If so, we'll know how to decode the transaction data to extract
    //      the creation code.
    //   4. If the `to` address is not a known factory, we trace the transaction to find the call
    //      that deployed the contract. Infura now supports `trace_call` so we can use that here.
    // Note that steps 1, 2, and 3 are implemented below. Step 4 is not implemented. Step 3 can also
    // be expanded to support more factories, or it can be removed entirely and we can always trace.
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
