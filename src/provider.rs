use ethers::{
    providers::{Http, Middleware, Provider},
    types::{Address, Bytes, Chain, TxHash},
};
use futures::future;
use std::{collections::HashMap, env, fs, path::PathBuf, str::FromStr, sync::Arc};

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

    pub async fn get_creation_code(&self, tx_hash: TxHash) -> ChainResponse<Bytes> {
        async fn get_input_data(provider: &Arc<Provider<Http>>, tx_hash: TxHash) -> Option<Bytes> {
            provider
                .get_transaction(tx_hash)
                .await
                .ok()
                .and_then(|tx| tx.map(|tx| if tx.to.is_none() { Some(tx.input) } else { None }))
                .flatten()
        }

        let futures = self.providers.iter().map(|(chain, provider)| async move {
            (*chain, get_input_data(provider, tx_hash).await)
        });
        let responses = future::join_all(futures).await.into_iter().collect::<HashMap<_, _>>();
        ChainResponse { responses }
    }

    pub fn compare_creation_code(
        &self,
        artifacts: Vec<PathBuf>,
        expected_creation_codes: &ChainResponse<Bytes>,
    ) -> ChainResponse<PathBuf> {
        fn compare(artifacts: Vec<PathBuf>, expected_creation_code: Bytes) -> Option<PathBuf> {
            for artifact in artifacts {
                let content = fs::read_to_string(&artifact).unwrap();
                let json: serde_json::Value = serde_json::from_str(&content).unwrap();
                if let Some(bytecode_value) = json.get("bytecode").unwrap().get("object") {
                    if let Some(bytecode_str) = bytecode_value.as_str() {
                        let bytecode = Bytes::from_str(bytecode_str).unwrap();
                        // TODO This check won't always work, e.g. constructor args, metadata hash,
                        // etc.
                        if bytecode == expected_creation_code {
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
                let expected_creation_code = expected_creation_codes.responses.get(chain).unwrap();
                if expected_creation_code.is_none() {
                    return (*chain, None)
                }
                let expected_creation_code = expected_creation_code.as_ref().unwrap();
                (*chain, compare(artifacts, expected_creation_code.clone()))
            })
            .collect::<HashMap<_, _>>();

        ChainResponse { responses }
    }
}
