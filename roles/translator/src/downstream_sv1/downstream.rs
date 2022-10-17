use crate::{downstream_sv1, ProxyResult};
use async_channel::{bounded, Receiver, Sender};
use async_std::{
    io::BufReader,
    net::{TcpListener, TcpStream},
    prelude::*,
    task,
};
use bigint;
use roles_logic_sv2::{
    common_properties::{IsDownstream, IsMiningDownstream},
    mining_sv2::{ExtendedExtranonce, Extranonce},
    utils::Mutex,
};
use std::{net::SocketAddr, sync::Arc};
use v1::{
    client_to_server, json_rpc, server_to_client,
    utils::{HexBytes, HexU32Be},
    IsServer,
};

/// Handles the sending and receiving of messages to and from an SV2 Upstream role (most typically
/// a SV2 Pool server).
#[derive(Debug)]
pub struct Downstream {
    /// List of authorized Downstream Mining Devices.
    authorized_names: Vec<String>,
    extranonce: ExtendedExtranonce,
    /// `extranonce1` to be sent to the Downstream in the SV1 `mining.subscribe` message response.
    // extranonce1: Vec<u8>,
    // extranonce2: Vec<u8>,
    /// Version rolling mask bits
    version_rolling_mask: Option<HexU32Be>,
    /// Minimum version rolling mask bits size
    version_rolling_min_bit: Option<HexU32Be>,
    /// Sends SV1 `mining.submit` message received from the SV1 Downstream to the Bridge for
    /// translation into a SV2 `SubmitSharesExtended`.
    submit_sender: Sender<(v1::client_to_server::Submit, ExtendedExtranonce)>,
    /// Sends message to the SV1 Downstream role.
    sender_outgoing: Sender<json_rpc::Message>,
    /// Difficulty target for SV1 Downstream.
    target: Arc<Mutex<Vec<u8>>>,
    /// True if this is the first job received from `Upstream`.
    first_job_received: bool,
}

impl Downstream {
    /// Instantiate a new `Downstream`.
    pub async fn new(
        stream: TcpStream,
        submit_sender: Sender<(v1::client_to_server::Submit, ExtendedExtranonce)>,
        mining_notify_receiver: Receiver<server_to_client::Notify>,
        extranonce: ExtendedExtranonce,
        last_notify: Arc<Mutex<Option<server_to_client::Notify>>>,
        target: Arc<Mutex<Vec<u8>>>,
    ) -> ProxyResult<Arc<Mutex<Self>>> {
        let stream = std::sync::Arc::new(stream);

        // Reads and writes from Downstream SV1 Mining Device Client
        let (socket_reader, socket_writer) = (stream.clone(), stream);
        let (sender_outgoing, receiver_outgoing) = bounded(10);

        let socket_writer_clone = socket_writer.clone();
        let _socket_writer_set_difficulty_clone = socket_writer.clone();
        // Used to send SV1 `mining.notify` messages to the Downstreams
        let _socket_writer_notify = socket_writer;

        //let extranonce: Vec<u8> = extranonce.try_into().unwrap();
        //let (extranonce1, _) = extranonce.split_at(extranonce.len() - extranonce2_size);

        let downstream = Arc::new(Mutex::new(Downstream {
            authorized_names: vec![],
            extranonce,
            //extranonce1: extranonce1.to_vec(),
            version_rolling_mask: None,
            version_rolling_min_bit: None,
            submit_sender,
            sender_outgoing,
            target: target.clone(),
            first_job_received: false,
        }));
        let self_ = downstream.clone();

        // Task to read from SV1 Mining Device Client socket via `socket_reader`. Depending on the
        // SV1 message received, a message response is sent directly back to the SV1 Downstream
        // role, or the message is sent upwards to the Bridge for translation into a SV2 message
        // and then sent to the SV2 Upstream role.
        task::spawn(async move {
            loop {
                // Read message from SV1 Mining Device Client socket
                let mut messages = BufReader::new(&*socket_reader).lines();
                // On message receive, parse to `json_rpc:Message` and send to Upstream
                // `Translator.receive_downstream` via `sender_upstream` done in
                // `send_message_upstream`.
                while let Some(incoming) = messages.next().await {
                    let incoming =
                        incoming.expect("Err reading next incoming message from SV1 Downstream");
                    //println!("\nInfo:: Down: Receiving: {:?}", &incoming);
                    let incoming: Result<json_rpc::Message, _> = serde_json::from_str(&incoming);
                    let incoming = incoming.expect("Err serializing incoming message from SV1 Downstream into JSON from `String`");
                    // Handle what to do with message
                    Self::handle_incoming_sv1(self_.clone(), incoming).await;
                }
            }
        });

        // Task to receive SV1 message responses to SV1 messages that do NOT need translation.
        // These response messages are sent directly to the SV1 Downstream role.
        task::spawn(async move {
            loop {
                let to_send = receiver_outgoing.recv().await.unwrap();
                let to_send = format!(
                    "{}\n",
                    serde_json::to_string(&to_send)
                        .expect("Err deserializing JSON message for SV1 Downstream into `String`")
                );
                //println!("\nInfo:: Down: Sending: {:?}", &to_send);
                (&*socket_writer_clone)
                    .write_all(to_send.as_bytes())
                    .await
                    .unwrap();
            }
        });

        let downstream_clone = downstream.clone();
        task::spawn(async move {
            let mut first_sent = false;
            loop {
                // Get receiver
                let is_a: bool = downstream_clone
                    .safe_lock(|d| !d.authorized_names.is_empty())
                    .unwrap();

                if is_a && !first_sent {
                    let target = target.safe_lock(|t| t.clone()).unwrap().to_vec();
                    let messsage = Self::get_set_difficulty(target);
                    // let target_2: bigint::U256 = target.safe_lock(|t| t.clone()).unwrap()[..]
                    //     .try_into()
                    //     .unwrap();
                    // let messsage = Self::get_set_difficulty(target_2);
                    Downstream::send_message_downstream(downstream_clone.clone(), messsage).await;

                    let sv1_mining_notify_msg =
                        last_notify.safe_lock(|s| s.clone()).unwrap().unwrap();
                    let messsage: json_rpc::Message = sv1_mining_notify_msg.try_into().unwrap();
                    Downstream::send_message_downstream(downstream_clone.clone(), messsage).await;
                    downstream_clone
                        .clone()
                        .safe_lock(|s| {
                            s.first_job_received = true;
                        })
                        .unwrap();
                    first_sent = true;
                } else if is_a {
                    let sv1_mining_notify_msg =
                        mining_notify_receiver.clone().recv().await.unwrap();
                    let messsage: json_rpc::Message = sv1_mining_notify_msg.try_into().unwrap();
                    Downstream::send_message_downstream(downstream_clone.clone(), messsage).await;
                }
            }
        });

        // Task to update the target and send a new `mining.set_difficulty` to the SV1 Downstream
        let downstream_clone = downstream.clone();
        task::spawn(async move {
            let target = downstream_clone.safe_lock(|t| t.target.clone()).unwrap();
            let mut last_target = target.safe_lock(|t| t.clone()).unwrap();
            loop {
                let target = downstream_clone
                    .clone()
                    .safe_lock(|t| t.target.clone())
                    .unwrap();
                let target = target.safe_lock(|t| t.clone()).unwrap();
                if target != last_target {
                    last_target = target;
                    let target_2 = last_target.to_vec();
                    let message = Self::get_set_difficulty(target_2);
                    // let target_2: bigint::U256 = last_target[..].try_into().unwrap();
                    // let message = Self::get_set_difficulty(target_2);
                    Downstream::send_message_downstream(downstream_clone.clone(), message).await;
                }
            }
        });

        Ok(downstream)
    }

    /// Converts target received by the `SetTarget` SV2 message from the Upstream role into the
    /// difficulty for the Downstream role sent via the SV1 `mining.set_difficulty` message.
    fn difficulty_from_target(target: Vec<u8>) -> f64 {
        // Convert target from Vec<u8> to U256 decimal representation (LE)
        let target_u256 = bigint::U256::from_little_endian(&target);

        // pdiff: 0x00000000FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF
        // https://en.bitcoin.it/wiki/Difficulty
        let pdiff = bigint::U256::from_dec_str(
            "26959946667150639794667015087019630673637144422540572481103610249215",
        )
        .unwrap();

        let diff = pdiff.overflowing_div(target_u256);
        let diff = diff.0.to_string();
        let diff: f64 = diff.parse().unwrap();
        println!("\nInfo:: Down: Setting difficulty to: {}", diff);
        diff
    }

    /// Converts target received by the `SetTarget` SV2 message from the Upstream role into the
    /// difficulty for the Downstream role and creates the SV1 `mining.set_difficulty` message to
    /// be sent to the Downstream role.
    fn get_set_difficulty(target: Vec<u8>) -> json_rpc::Message {
        let value = Downstream::difficulty_from_target(target);
        let set_target = v1::methods::server_to_client::SetDifficulty { value };
        let message: json_rpc::Message = set_target.try_into().unwrap();
        message
    }

    /// Accept connections from one or more SV1 Downstream roles (SV1 Mining Devices) and create a
    /// new `Downstream` for each connection.
    pub async fn accept_connections(
        downstream_addr: SocketAddr,
        submit_sender: Sender<(v1::client_to_server::Submit, ExtendedExtranonce)>,
        receiver_mining_notify: Receiver<server_to_client::Notify>,
        mut extended_extranonce: ExtendedExtranonce,
        last_notify: Arc<Mutex<Option<server_to_client::Notify>>>,
        target: Arc<Mutex<Vec<u8>>>,
    ) {
        task::spawn(async move {
            let downstream_listener = TcpListener::bind(downstream_addr).await.unwrap();
            let mut downstream_incoming = downstream_listener.incoming();
            while let Some(stream) = downstream_incoming.next().await {
                let stream = stream.expect("Err on SV1 Downstream connection stream");
                println!(
                    "\nPROXY SERVER - ACCEPTING FROM DOWNSTREAM: {}\n",
                    stream.peer_addr().unwrap()
                );
                let server = Downstream::new(
                    stream,
                    submit_sender.clone(),
                    receiver_mining_notify.clone(),
                    extended_extranonce.clone(),
                    last_notify.clone(),
                    target.clone(),
                )
                .await
                .unwrap();
                Arc::new(Mutex::new(server));
            }
        });
    }

    /// As SV1 messages come in, determines if the message response needs to be translated to SV2
    /// and sent to the `Upstream`, or if a direct response can be sent back by the `Translator`
    /// (SV1 and SV2 protocol messages are NOT 1-to-1).
    async fn handle_incoming_sv1(self_: Arc<Mutex<Self>>, message_sv1: json_rpc::Message) {
        // `handle_message` in `IsServer` trait + calls `handle_request`
        // TODO: Map err from V1Error to Error::V1Error
        let response = self_.safe_lock(|s| s.handle_message(message_sv1)).unwrap();
        match response {
            Ok(res) => {
                if let Some(r) = res {
                    // If some response is received, indicates no messages translation is needed
                    // and response should be sent directly to the SV1 Downstream. Otherwise,
                    // message will be sent to the upstream Translator to be translated to SV2 and
                    // forwarded to the `Upstream`
                    // let sender = self_.safe_lock(|s| s.connection.sender_upstream)
                    Self::send_message_downstream(self_, r.into()).await;
                } else {
                    // If None response is received, indicates this SV1 message received from the
                    // Downstream MD is passed to the `Translator` for translation into SV2
                }
            }
            Err(e) => {
                panic!("`{:?}`", e);
            }
        }
    }

    /// Send SV1 response message that is generated by `Downstream` (as opposed to being received
    /// by `Bridge`) to be written to the SV1 Downstream role.
    async fn send_message_downstream(self_: Arc<Mutex<Self>>, response: json_rpc::Message) {
        let sender = self_.safe_lock(|s| s.sender_outgoing.clone()).unwrap();
        sender.send(response).await.unwrap();
    }
}

/// Implements `IsServer` for `Downstream` to handle the SV1 messages.
impl IsServer for Downstream {
    /// Handle the incoming `mining.configure` message which is received after a Downstream role is
    /// subscribed and authorized. Contains the version rolling mask parameters.
    fn handle_configure(
        &mut self,
        request: &client_to_server::Configure,
    ) -> (Option<server_to_client::VersionRollingParams>, Option<bool>) {
        println!("\nInfo:: Down: Configuring");
        println!("Debug:: Down: Handling mining.configure: {:?}", &request);
        self.version_rolling_mask = Some(downstream_sv1::new_version_rolling_mask());
        self.version_rolling_min_bit = Some(downstream_sv1::new_version_rolling_min());
        (
            Some(server_to_client::VersionRollingParams::new(
                self.version_rolling_mask.clone().unwrap(),
                self.version_rolling_min_bit.clone().unwrap(),
            )),
            Some(false),
        )
    }

    /// Handle the response to a `mining.subscribe` message received from the client.
    /// The subscription messages are erroneous and just used to conform the SV1 protocol spec.
    /// Because no one unsubscribed in practice, they just unplug their machine.
    fn handle_subscribe(&self, request: &client_to_server::Subscribe) -> Vec<(String, String)> {
        println!("\nInfo:: Down: Subscribing");
        println!("Debug:: Down: Handling mining.subscribe: {:?}", &request);

        let set_difficulty_sub = (
            "mining.set_difficulty".to_string(),
            downstream_sv1::new_subscription_id(),
        );
        let notify_sub = (
            "mining.notify".to_string(),
            "ae6812eb4cd7735a302a8a9dd95cf71f".to_string(),
        );

        vec![set_difficulty_sub, notify_sub]
    }

    /// Any numbers of workers may be authorized at any time during the session. In this way, a
    /// large number of independent Mining Devices can be handled with a single SV1 connection.
    /// https://bitcoin.stackexchange.com/questions/29416/how-do-pool-servers-handle-multiple-workers-sharing-one-connection-with-stratum
    fn handle_authorize(&self, request: &client_to_server::Authorize) -> bool {
        println!("\nInfo:: Down: Authorizing");
        println!("Debug:: Down: Handling mining.authorize: {:?}", &request);
        true
    }

    /// When miner find the job which meets requested difficulty, it can submit share to the server.
    /// Only [Submit](client_to_server::Submit) requests for authorized user names can be submitted.
    fn handle_submit(&self, request: &client_to_server::Submit) -> bool {
        //println!("\nInfo:: Down: Submitting Share");
        //println!("Debug:: Down: Handling mining.submit: {:?}", &request);

        // TODO: Check if receiving valid shares by adding diff field to Downstream

        if self.first_job_received {
            let to_send = (request.clone(), self.extranonce.clone());
            self.submit_sender.try_send(to_send).unwrap();
        };
        true
    }

    /// Indicates to the server that the client supports the mining.set_extranonce method.
    fn handle_extranonce_subscribe(&self) {}

    /// Checks if a Downstream role is authorized.
    fn is_authorized(&self, name: &str) -> bool {
        self.authorized_names.contains(&name.to_string())
    }

    /// Authorizes a Downstream role.
    fn authorize(&mut self, name: &str) {
        self.authorized_names.push(name.to_string());
    }

    /// Sets the `extranonce1` field sent in the SV1 `mining.notify` message to the value specified
    /// by the SV2 `OpenExtendedMiningChannelSuccess` message sent from the Upstream role.
    fn set_extranonce1(&mut self, _extranonce1: Option<HexBytes>) -> HexBytes {
        let extranonce1: Vec<u8> = self.extranonce.upstream_part().try_into().unwrap();
        extranonce1.try_into().unwrap()
    }

    /// Returns the `Downstream`'s `extranonce1` value.
    fn extranonce1(&self) -> HexBytes {
        let downstream_ext: Vec<u8> = self
            .extranonce
            .without_upstream_part(None)
            .unwrap()
            .try_into()
            .unwrap();
        downstream_ext.try_into().unwrap()
    }

    /// Sets the `extranonce2_size` field sent in the SV1 `mining.notify` message to the value
    /// specified by the SV2 `OpenExtendedMiningChannelSuccess` message sent from the Upstream role.
    fn set_extranonce2_size(&mut self, _extra_nonce2_size: Option<usize>) -> usize {
        self.extranonce.get_range2_len()
    }

    /// Returns the `Downstream`'s `extranonce2_size` value.
    fn extranonce2_size(&self) -> usize {
        self.extranonce.get_range2_len()
    }

    /// Returns the version rolling mask.
    fn version_rolling_mask(&self) -> Option<HexU32Be> {
        self.version_rolling_mask.clone()
    }

    /// Sets the version rolling mask.
    fn set_version_rolling_mask(&mut self, mask: Option<HexU32Be>) {
        self.version_rolling_mask = mask;
    }

    /// Sets the minimum version rolling bit.
    fn set_version_rolling_min_bit(&mut self, mask: Option<HexU32Be>) {
        self.version_rolling_min_bit = mask
    }

    fn notify(&mut self) -> Result<json_rpc::Message, v1::error::Error> {
        unreachable!()
    }
}

impl IsMiningDownstream for Downstream {}

impl IsDownstream for Downstream {
    fn get_downstream_mining_data(
        &self,
    ) -> roles_logic_sv2::common_properties::CommonDownstreamData {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gets_difficulty_from_target() {
        let target = vec![
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 128, 255, 127,
            0, 0, 0, 0, 0,
        ];
        let actual = Downstream::difficulty_from_target(target);
        let expect = 512.0;
        assert_eq!(actual, expect);
    }
}
