use ethers::{
    providers::{Http, Middleware, Provider},
    types::{Bytes, Chain, TxHash},
};
use std::{env, sync::Arc};

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

// pub struct MultiChainProvider {
//     gnosis_chain: Arc<Provider<Http>>,
//     goerli: Arc<Provider<Http>>,
//     mainnet: Arc<Provider<Http>>,
//     optimism: Arc<Provider<Http>>,
//     polygon: Arc<Provider<Http>>,
// }

// pub struct ChainResponse<T> {
//     gnosis_chain: Option<T>,
//     goerli: Option<T>,
//     mainnet: Option<T>,
//     optimism: Option<T>,
//     polygon: Option<T>,
// }

// impl Default for MultiChainProvider {
//     fn default() -> Self {
//         Self::new()
//     }
// }

// impl MultiChainProvider {
//     pub fn new() -> Self {
//         Self {
//             gnosis_chain: Arc::new(
//                 Provider::<Http>::try_from(env::var("GNOSIS_CHAIN_RPC_URL").unwrap()).unwrap(),
//             ),
//             goerli: Arc::new(
//                 Provider::<Http>::try_from(env::var("GOERLI_RPC_URL").unwrap()).unwrap(),
//             ),
//             mainnet: Arc::new(
//                 Provider::<Http>::try_from(env::var("MAINNET_RPC_URL").unwrap()).unwrap(),
//             ),
//             optimism: Arc::new(
//                 Provider::<Http>::try_from(env::var("OPTIMISM_RPC_URL").unwrap()).unwrap(),
//             ),
//             polygon: Arc::new(
//                 Provider::<Http>::try_from(env::var("POLYGON_RPC_URL").unwrap()).unwrap(),
//             ),
//         }
//     }

//     pub async fn get_creation_code(&self, tx_hash: TxHash) -> ChainResponse<Bytes> {
//         async fn get_input_data(provider: &Arc<Provider<Http>>, tx_hash: TxHash) -> Option<Bytes>
// {             provider
//                 .get_transaction(tx_hash)
//                 .await
//                 .ok()
//                 .and_then(|tx| tx.map(|tx| if tx.to.is_none() { Some(tx.input) } else { None }))
//                 .flatten()
//         }

//         let gnosis_chain_future = get_input_data(&self.gnosis_chain, tx_hash);
//         let goerli_future = get_input_data(&self.goerli, tx_hash);
//         let mainnet_future = get_input_data(&self.mainnet, tx_hash);
//         let optimism_future = get_input_data(&self.optimism, tx_hash);
//         let polygon_future = get_input_data(&self.polygon, tx_hash);

//         let (gnosis_chain, goerli, mainnet, optimism, polygon) = futures::join!(
//             gnosis_chain_future,
//             goerli_future,
//             mainnet_future,
//             optimism_future,
//             polygon_future
//         );
//         ChainResponse { gnosis_chain, goerli, mainnet, optimism, polygon }
//     }
// }
