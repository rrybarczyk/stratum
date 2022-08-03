use crate::downstream_sv1::DownstreamConnection;
use async_std::net::TcpStream;

use async_channel::{bounded, Receiver, Sender};
use async_std::{io::BufReader, prelude::*, task};
use roles_logic_sv2::common_properties::{IsDownstream, IsMiningDownstream};
use roles_logic_sv2::utils::Mutex;
use std::sync::Arc;
use v1::json_rpc;

/// Handles the sending and receiving of messages to and from an SV2 Upstream role (most typically
/// a SV2 Pool server).
#[derive(Debug)]
pub(crate) struct Downstream {
    connection: DownstreamConnection,
}
// new task loops through receiver upstream is sending something, if so use sender outgoing and
// transform to sv1 messages then use sender outgoing to send to the socket
impl IsMiningDownstream for Downstream {}
impl IsDownstream for Downstream {
    fn get_downstream_mining_data(
        &self,
    ) -> roles_logic_sv2::common_properties::CommonDownstreamData {
        todo!()
    }
}

impl Downstream {
    pub async fn new(
        stream: TcpStream,
        sender_upstream: Sender<json_rpc::Message>,
        receiver_upstream: Receiver<json_rpc::Message>,
    ) -> Arc<Mutex<Self>> {
        let stream = std::sync::Arc::new(stream);

        let (socket_reader, socket_writer) = (stream.clone(), stream);
        let (sender_incoming, receiver_incoming) = bounded(10);
        let (sender_outgoing, receiver_outgoing) = bounded(10);

        let connection = DownstreamConnection {
            sender_outgoing,
            receiver_incoming,
            sender_upstream,
            receiver_upstream,
        };

        let dowstream = Arc::new(Mutex::new(Downstream { connection }));

        let self_ = dowstream.clone();
        task::spawn(async move {
            loop {
                let to_send = receiver_outgoing.recv().await.unwrap();
                let to_send = format!("{}\n", serde_json::to_string(&to_send).unwrap());
                (&*socket_writer)
                    .write_all(to_send.as_bytes())
                    .await
                    .unwrap();
            }
        });
        // Task to listen on Downstream socket for incoming messages
        task::spawn(async move {
            let mut messages = BufReader::new(&*socket_reader).lines();
            while let Some(incoming) = messages.next().await {
                let incoming = incoming.unwrap();
                let incoming: Result<json_rpc::Message, _> = serde_json::from_str(&incoming);
                match incoming {
                    Ok(message) => {
                        let to_send = Self::parse_message(self_.clone(), message).await;
                        match to_send {
                            Some(message) => {
                                // TODO: add relay_message fn
                                // self.relay_message(m).await;
                                // Sends Downstream messages received from socket to downstream
                                Self::send_message(self_.clone(), message).await;
                            }
                            None => (),
                        }
                    }
                    Err(_) => (),
                }
            }
        });

        dowstream
    }

    #[allow(clippy::single_match)]
    async fn parse_message(
        self_: Arc<Mutex<Self>>,
        incoming_message: json_rpc::Message,
    ) -> Option<json_rpc::Message> {
        todo!()
    }

    /// Translates the SV1 message into an SV2 message
    async fn relay_message(self_: Arc<Mutex<Self>>, msg: json_rpc::Message) {
        let sender = self_
            .safe_lock(|s| s.connection.sender_outgoing.clone())
            .unwrap();
        sender.send(msg).await.unwrap()
    }

    /// Sends SV1 message to the Downstream client (most typically a SV1 Mining Device).
    async fn send_message(self_: Arc<Mutex<Self>>, msg: json_rpc::Message) {
        let sender = self_
            .safe_lock(|s| s.connection.sender_outgoing.clone())
            .unwrap();
        sender.send(msg).await.unwrap()
    }
}