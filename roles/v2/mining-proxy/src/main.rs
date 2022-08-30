//! Configurable Sv2 it support extended and group channel
//! Upstream means another proxy or a pool
//! Downstream means another proxy or a mining device
//!
//! UpstreamMining is the trait that a proxy must implement in order to
//! understant Downstream mining messages.
//!
//! DownstreamMining is the trait that a proxy must implement in order to
//! understand Upstream mining messages
//!
//! Same thing for DownstreamCommon and UpstreamCommon but for common messages
//!
//! DownstreamMiningNode implement both UpstreamMining and UpstreamCommon
//!
//! UpstreamMiningNode implement both DownstreamMining and DownstreamCommon
//!
//! A Downstream that signal the capacity to handle group channels can open more than one channel.
//! A Downstream that signal the incapacity to handle group channels can open only one channel.
//!
mod lib;
use std::net::SocketAddr;

use lib::upstream_mining::UpstreamMiningNode;
use once_cell::sync::{Lazy, OnceCell};
use serde::Deserialize;

use roles_logic_sv2::{
    routing_logic::{CommonRoutingLogic, MiningProxyRoutingLogic, MiningRoutingLogic},
    selectors::{GeneralMiningSelector, UpstreamMiningSelctor},
    utils::{Id, Mutex},
};
use std::{collections::HashMap, sync::Arc};

type RLogic = MiningProxyRoutingLogic<
    crate::lib::downstream_mining::DownstreamMiningNode,
    crate::lib::upstream_mining::UpstreamMiningNode,
    crate::lib::upstream_mining::ProxyRemoteSelector,
>;

pub fn max_supported_version() -> u16 {
    let config_file = std::fs::read_to_string("proxy-config.toml").unwrap();
    let config: Config = toml::from_str(&config_file).unwrap();
    config.max_supported_version
}
pub fn min_supported_version() -> u16 {
    let config_file = std::fs::read_to_string("proxy-config.toml").unwrap();
    let config: Config = toml::from_str(&config_file).unwrap();
    config.min_supported_version
}

/// Panic whene we are looking one of this 2 global mutex would force the proxy to go down as every
/// part of the program depend on them.
/// SAFTEY note: we use global mutable memory instead of a dedicated struct that use a dedicated
/// task to change the mutable state and communicate with the other parts of the program via
/// messages cause it is impossible for a task to panic while is using one of the two below Mutex.
/// So it make sense to use shared mutable memory to lower the complexity of the codebase and to
/// have some performance gain.
static ROUTING_LOGIC: OnceCell<Mutex<RLogic>> = OnceCell::new();
static JOB_ID_TO_UPSTREAM_ID: Lazy<Mutex<HashMap<u32, u32>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

async fn initialize_upstreams() {
    let upstreams = ROUTING_LOGIC
        .get()
        .expect("BUG: ROUTING_LOGIC has not been set yet")
        .safe_lock(|r_logic| r_logic.upstream_selector.upstreams.clone())
        .unwrap();
    crate::lib::upstream_mining::scan(upstreams).await;
}

pub fn get_routing_logic() -> MiningRoutingLogic<
    crate::lib::downstream_mining::DownstreamMiningNode,
    crate::lib::upstream_mining::UpstreamMiningNode,
    crate::lib::upstream_mining::ProxyRemoteSelector,
    RLogic,
> {
    MiningRoutingLogic::Proxy(
        ROUTING_LOGIC
            .get()
            .expect("BUG: ROUTING_LOGIC was not set yet"),
    )
}
pub fn get_common_routing_logic() -> CommonRoutingLogic<RLogic> {
    CommonRoutingLogic::Proxy(
        ROUTING_LOGIC
            .get()
            .expect("BUG: ROUTING_LOGIC was not set yet"),
    )
}

pub fn upstream_from_job_id(job_id: u32) -> Option<Arc<Mutex<UpstreamMiningNode>>> {
    let upstream_id: u32;
    upstream_id = JOB_ID_TO_UPSTREAM_ID
        .safe_lock(|x| *x.get(&job_id).unwrap())
        .unwrap();
    ROUTING_LOGIC
        .get()
        .expect("BUG: ROUTING_LOGIC was not set yet")
        .safe_lock(|rlogic| rlogic.upstream_selector.get_upstream(upstream_id))
        .unwrap()
}

pub fn add_job_id(job_id: u32, up_id: u32, prev_job_id: Option<u32>) {
    if let Some(prev_job_id) = prev_job_id {
        JOB_ID_TO_UPSTREAM_ID
            .safe_lock(|x| x.remove(&prev_job_id))
            .unwrap();
    }
    JOB_ID_TO_UPSTREAM_ID
        .safe_lock(|x| x.insert(job_id, up_id))
        .unwrap();
}

#[derive(Debug, Deserialize)]
pub struct UpstreamValues {
    address: String,
    port: u16,
    pub_key: codec_sv2::noise_sv2::formats::EncodedEd25519PublicKey,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    upstreams: Vec<UpstreamValues>,
    listen_address: String,
    listen_mining_port: u16,
    max_supported_version: u16,
    min_supported_version: u16,
}

pub fn initialize_r_logic(upstreams: &[UpstreamValues]) -> RLogic {
    let job_ids = Arc::new(Mutex::new(Id::new()));
    let upstream_mining_nodes: Vec<Arc<Mutex<UpstreamMiningNode>>> = upstreams
        .iter()
        .enumerate()
        .map(|(index, upstream)| {
            let socket = SocketAddr::new(upstream.address.parse().unwrap(), upstream.port);
            Arc::new(Mutex::new(UpstreamMiningNode::new(
                index as u32,
                socket,
                upstream.pub_key.clone().into_inner().to_bytes(),
                job_ids.clone(),
            )))
        })
        .collect();
    //crate::lib::upstream_mining::scan(upstream_mining_nodes.clone()).await;
    let upstream_selector = GeneralMiningSelector::new(upstream_mining_nodes);
    MiningProxyRoutingLogic {
        upstream_selector,
        downstream_id_generator: Id::new(),
        downstream_to_upstream_map: std::collections::HashMap::new(),
    }
}

mod args {
    use std::path::PathBuf;

    #[derive(Debug)]
    pub struct Args {
        pub config_path: PathBuf,
    }

    enum ArgsState {
        Next,
        ExpectPath,
        Done,
    }

    enum ArgsResult {
        Config(PathBuf),
        None,
        Help(String),
    }

    impl Args {
        const DEFAULT_CONFIG_PATH: &'static str = "proxy-config.toml";

        pub fn from_args() -> Result<Self, String> {
            let cli_args = std::env::args();

            let config_path = cli_args
                .scan(ArgsState::Next, |state, item| {
                    match std::mem::replace(state, ArgsState::Done) {
                        ArgsState::Next => match item.as_str() {
                            "-c" | "--config" => {
                                *state = ArgsState::ExpectPath;
                                Some(ArgsResult::None)
                            }
                            "-h" | "--help" => Some(ArgsResult::Help(format!(
                                "Usage: -h/--help, -c/--config <path|default {}>",
                                Self::DEFAULT_CONFIG_PATH
                            ))),
                            _ => {
                                *state = ArgsState::Next;

                                Some(ArgsResult::None)
                            }
                        },
                        ArgsState::ExpectPath => Some(ArgsResult::Config(PathBuf::from(item))),
                        ArgsState::Done => None,
                    }
                })
                .last();
            let config_path = match config_path {
                Some(ArgsResult::Config(p)) => p,
                Some(ArgsResult::Help(h)) => return Err(h),
                _ => PathBuf::from(Self::DEFAULT_CONFIG_PATH),
            };
            Ok(Self { config_path })
        }
    }
}

/// 1. the proxy scan all the upstreams and map them
/// 2. donwstream open a connetcion with proxy
/// 3. downstream send SetupConnection
/// 4. a mining_channle::Upstream is created
/// 5. upstream_mining::UpstreamMiningNodes is used to pair this downstream with the most suitable
///    upstream
/// 6. mining_channle::Upstream create a new downstream_mining::DownstreamMiningNode embedding
///    itself in it
/// 7. normal operation between the paired downstream_mining::DownstreamMiningNode and
///    upstream_mining::UpstreamMiningNode begin
#[tokio::main]
async fn main() {
    let args = match args::Args::from_args() {
        Ok(cfg) => cfg,
        Err(help) => {
            println!("{}", help);
            return;
        }
    };

    // Scan all the upstreams and map them
    let config_file = std::fs::read_to_string(args.config_path).expect("TODO: Error handling");
    let config = match toml::from_str::<Config>(&config_file) {
        Ok(cfg) => cfg,
        Err(e) => {
            println!("Failed to parse config file: {}", e);
            return;
        }
    };
    ROUTING_LOGIC
        .set(Mutex::new(initialize_r_logic(&config.upstreams)))
        .expect("BUG: Failed to set ROUTING_LOGIC");

    println!("PROXY INITIALIZING");
    initialize_upstreams().await;

    // Wait for downstream connection
    let socket = SocketAddr::new(
        config.listen_address.parse().unwrap(),
        config.listen_mining_port,
    );
    println!("PROXY INITIALIZED");
    crate::lib::downstream_mining::listen_for_downstream_mining(socket).await;
}
