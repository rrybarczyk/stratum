mod downstream_sv1;
mod error;
mod proxy;
mod upstream_sv2;

use async_channel::bounded;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

pub const UPSTREAM_IP: &str = "127.0.0.1";
pub const UPSTREAM_PORT: u16 = 34254;
pub const LISTEN_ADDR: &str = "127.0.0.1:34255";
/// TODO: Authority public key used to authorize with Upstream is hardcoded, but should be read
/// in via a proxy-config.toml.
const AUTHORITY_PUBLIC_KEY: [u8; 32] = [
    215, 11, 47, 78, 34, 232, 25, 192, 195, 168, 170, 209, 95, 181, 40, 114, 154, 226, 176, 190,
    90, 169, 238, 89, 191, 183, 97, 63, 194, 119, 11, 31,
];

#[async_std::main]
async fn main() {
    // `sender_submit_from_sv1` sender is used by `Downstream` to send a `mining.submit` message to
    // `Bridge` via the `recv_submit_from_sv1` receiver
    // (Sender<v1::client_to_server::Submit>, Receiver<Submit>)
    let (sender_submit_from_sv1, recv_submit_from_sv1) = bounded(10);
    // `sender_submit_to_sv2` sender is used by `Bridge` to send a `SubmitSharesExtended` message
    // to `Upstream` via the `recv_submit_to_sv2` receiver
    // (Sender<SubmitSharesExtended<'static>>, Receiver<SubmitSharesExtended<'static>>)
    let (sender_submit_to_sv2, recv_submit_to_sv2) = bounded(10);

    // `sender_new_prev_hash` sender is used by `Upstream` to send a `SetNewPrevHash` to `Bridge`
    // via the `recv_new_prev_hash` receiver
    // (Sender<SetNewPrevHash<'static>>, Receiver<SetNewPrevHash<'static>>)
    let (sender_new_prev_hash, recv_new_prev_hash) = bounded(10);

    // `sender_new_extended_mining_job` sender is used by `Upstream` to send a
    // `NewExtendedMiningJob` to `Bridge` via the `recv_new_extended_mining_job` receiver
    // (Sender<NewExtendedMiningJob<'static>>, Receiver<NewExtendedMiningJob<'static>>)
    let (sender_new_extended_mining_job, recv_new_extended_mining_job) = bounded(10);

    // TODO add a channel to send new jobs from Bridge to Downstream

    // Format `Upstream` connection address
    let upstream_addr = SocketAddr::new(
        IpAddr::from_str(crate::UPSTREAM_IP).unwrap(),
        crate::UPSTREAM_PORT,
    );

    // Instantiate a new `Upstream`
    let upstream = upstream_sv2::Upstream::new(
        upstream_addr,
        crate::AUTHORITY_PUBLIC_KEY,
        recv_submit_to_sv2,
        sender_new_prev_hash,
        sender_new_extended_mining_job,
    )
    .await;
    // Connects to the SV2 Upstream role
    upstream_sv2::Upstream::connect(upstream.clone()).await;
    // Start receiving messages from the SV2 Upstream role
    upstream_sv2::Upstream::parse_incoming(upstream.clone());
    // Start receiving submit from the SV1 Downstream role
    upstream_sv2::Upstream::on_submit(upstream.clone());

    // Instantiates a new `Bridge` and begins handling incoming messages
    proxy::Bridge::new(
        recv_submit_from_sv1,
        sender_submit_to_sv2,
        recv_new_prev_hash,
        recv_new_extended_mining_job,
    )
    .start();

    // Accept connections from one or more SV1 Downstream roles (SV1 Mining Devices)
    downstream_sv1::Downstream::accept_connections(sender_submit_from_sv1);
    // async_std::task::spawn(async {
    //proxy::Bridge::initiate().await;
    // })
    // .await;
    loop {}
}
