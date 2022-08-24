///
/// Translator is a Proxy server sits between a Downstream role (most typically a SV1 Mining
/// Device, but could also be a SV1 Proxy server) and an Upstream role (most typically a SV2 Pool
/// server, but could also be a SV2 Proxy server). It accepts and sends messages between the SV1
/// Downstream role and the SV2 Upstream role, translating the messages into the appropriate
/// protocol.
///
/// **Translator starts**
///
/// 1. Connects to SV2 Upstream role.
///    a. Sends a SV2 `SetupConnection` message to the SV2 Upstream role + receives a SV2
///       `SetupConnectionSuccess` or `SetupConnectionError` message in response.
///    b.  SV2 Upstream role immediately sends a SV2 `SetNewPrevHash` + `NewExtendedMiningJob`
///        message.
///    c. If connection was successful, sends a SV2 `OpenExtendedMiningChannel` message to the SV2
///       Upstream role + receives a SV2 `OpenExtendedMiningChannelSuccess` or
///       `OpenMiningChannelError` message in response.
///
/// 2. Meanwhile, Translator is listening for a SV1 Downstream role to connect. On connection:
///    a. Receives a SV1 `mining.subscribe` message from the SV1 Downstream role + sends a response
///       with a SV1 `mining.set_difficulty` + `mining.notify` which the Translator builds using
///       the SV2 `SetNewPrevHash` + `NewExtendedMiningJob` messages received from the SV2 Upstream
///       role.
///
/// 3. Translator waits for the SV1 Downstream role to find a valid share submission.
///    a. It receives this share submission via a SV1 `mining.submit` message + translates it into a
///       SV2 `SubmitSharesExtended` message which is then sent to the SV2 Upstream role + receives
///       a SV2 `SubmitSharesSuccess` or `SubmitSharesError` message in response.
///    b. This keeps happening until a new Bitcoin block is confirmed on the network, making this
///       current job's PrevHash stale.
///
/// 4. When a new block is confirmed on the Bitcoin network, the Translator sends a fresh job to
///    the SV1 Downstream role.
///    a. The SV2 Upstream role immediately sends the Translator a fresh SV2 `SetNewPrevHash`
///       followed by a `NewExtendedMiningJob` message.
///    b. Once the Translator receives BOTH messages, it translates them into a SV1 `mining.notify`
///       message + sends to the SV1 Downstream role.
///    c. The SV1 Downstream role begins finding a new valid share submission + Step 3 commences
///       again.
///
use crate::{
    downstream_sv1::Downstream,
    proxy::{DownstreamTranslator, UpstreamTranslator},
    upstream_sv2::{EitherFrame, Message, StdFrame, Upstream},
};
use async_channel::{bounded, Receiver, Sender};
use async_std::{net::TcpListener, prelude::*, task};
use codec_sv2::Frame;
use core::convert::TryInto;
use roles_logic_sv2::{
    parsers::{JobNegotiation, Mining},
    utils::Mutex,
};
use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::Arc,
};
use v1::json_rpc;

#[derive(Clone)]
pub(crate) struct Translator {
    pub(crate) downstream_translator: DownstreamTranslator,
    pub(crate) upstream_translator: UpstreamTranslator,
}

impl Translator {
    pub async fn new() -> Self {
        // A channel for the `Downstream` to send to the `Translator` and for the `Translator` to
        // receive from the `Downstream`
        let (sender_for_downstream, receiver_downstream_for_proxy): (
            Sender<json_rpc::Message>,
            Receiver<json_rpc::Message>,
        ) = bounded(10);
        // A channel for the `Translator` to send to the `Downstream` and for the `Downstream` to
        // receive from the `Translator`:
        let (sender_downstream_for_proxy, receiver_for_downstream): (
            Sender<json_rpc::Message>,
            Receiver<json_rpc::Message>,
        ) = bounded(10);
        // A channel for the `Upstream` to send to the `Translator` and for the `Translator` to
        // receive from the `Upstream`
        let (sender_for_upstream, receiver_upstream_for_proxy): (
            Sender<EitherFrame>,
            Receiver<EitherFrame>,
        ) = bounded(10);
        // A channel for the `Translator` to send to the `Upstream` and for the `Upstream` to
        // receive from the `Translator`
        let (sender_upstream_for_proxy, receiver_for_upstream): (
            Sender<EitherFrame>,
            Receiver<EitherFrame>,
        ) = bounded(10);

        let downstream_translator =
            DownstreamTranslator::new(sender_downstream_for_proxy, receiver_downstream_for_proxy);
        let upstream_translator =
            UpstreamTranslator::new(sender_upstream_for_proxy, receiver_upstream_for_proxy);
        let translator = Translator {
            downstream_translator,
            upstream_translator,
        };
        // Listen for SV1 Downstream(s) + SV2 Upstream, process received messages + send
        // accordingly
        let translator_clone_listen = translator.clone();
        translator_clone_listen.listen().await;

        // Connect to SV1 Downstream(s) + SV2 Upstream
        let translator_clone_connect = translator.clone();
        translator_clone_connect
            .connect(
                sender_for_downstream,
                receiver_for_downstream,
                sender_for_upstream,
                receiver_for_upstream,
            )
            .await;

        translator
    }

    /// Connect to SV1 Downstream(s) (SV1 Mining Device) + SV2 Upstream (SV2 Pool).
    async fn connect(
        self,
        sender_for_downstream: Sender<json_rpc::Message>,
        receiver_for_downstream: Receiver<json_rpc::Message>,
        sender_for_upstream: Sender<EitherFrame>,
        receiver_for_upstream: Receiver<EitherFrame>,
    ) {
        println!("CONNECTING...\n");
        // Accept connection from one SV2 Upstream role (SV2 Pool)
        Translator::accept_connection_upstream(sender_for_upstream, receiver_for_upstream).await;

        // Accept connections from one or more SV1 Downstream roles (SV1 Mining Devices)
        Translator::accept_connection_downstreams(
            sender_for_downstream.clone(),
            receiver_for_downstream.clone(),
        )
        .await;
    }

    /// Listen for SV1 Downstream(s) + SV2 Upstream, process received messages + send accordingly.
    async fn listen(self) {
        println!("\nLISTENING...\n");
        // Spawn task to listen for incoming messages from SV1 Downstream
        let translator_clone_downstream = self.clone();
        translator_clone_downstream.listen_downstream().await;

        // Spawn task to listen for incoming messages from SV2 Upstream
        let translator_clone_upstream = self.clone();
        translator_clone_upstream.listen_upstream().await;
    }

    /// Accept connection from one SV2 Upstream role (SV2 Pool).
    /// TODO: Authority public key used to authorize with Upstream is hardcoded, but should be read
    /// in via a proxy-config.toml.
    async fn accept_connection_upstream(
        sender_for_upstream: Sender<EitherFrame>,
        receiver_for_upstream: Receiver<EitherFrame>,
    ) {
        let upstream_addr = SocketAddr::new(
            IpAddr::from_str(crate::UPSTREAM_IP).unwrap(),
            crate::UPSTREAM_PORT,
        );
        let _upstream = Upstream::new(
            upstream_addr,
            crate::AUTHORITY_PUBLIC_KEY,
            sender_for_upstream,
            receiver_for_upstream,
        )
        .await;
    }

    /// Accept connections from one or more SV1 Downstream roles (SV1 Mining Devices).
    async fn accept_connection_downstreams(
        sender_for_downstream: Sender<json_rpc::Message>,
        receiver_for_downstream: Receiver<json_rpc::Message>,
    ) {
        let downstream_listener = TcpListener::bind(crate::LISTEN_ADDR).await.unwrap();
        let mut downstream_incoming = downstream_listener.incoming();
        while let Some(stream) = downstream_incoming.next().await {
            let stream = stream.unwrap();
            println!(
                "\nPROXY SERVER - ACCEPTING FROM DOWNSTREAM: {}\n",
                stream.peer_addr().unwrap()
            );
            let server = Downstream::new(
                stream,
                sender_for_downstream.clone(),
                receiver_for_downstream.clone(),
            )
            .await;
            Arc::new(Mutex::new(server));
        }
    }

    /// Spawn task to listen for incoming messages from SV1 Downstream.
    /// Spawned task waits to receive a message from `Downstream.connection.sender_upstream`,
    /// then parses the message + translates to SV2. Then the `Translator.sender_upstream` sends
    /// the SV2 message to the `Upstream.receiver_downstream`.
    async fn listen_downstream(mut self) {
        task::spawn(async move {
            println!("TP LISTENING FOR INCOMING SV1 MSG FROM TD\n");
            loop {
                let message_sv1: json_rpc::Message =
                    self.downstream_translator.receiver.recv().await.unwrap();
                let message_sv2: EitherFrame = self.parse_sv1_to_sv2(message_sv1);
                self.upstream_translator.send_sv2(message_sv2).await;
            }
        });
    }

    /// Spawn task to listen for incoming messages from SV2 Upstream.
    /// Spawned task waits to receive a message from `Upstream.connection.sender_downstream`,
    /// then parses the message + translates to SV1. Then the
    /// `Translator.downstream_translator.sender` sends the SV1 message to the
    /// `Downstream.receiver_upstream`.
    async fn listen_upstream(mut self) {
        task::spawn(async move {
            println!("TP LISTENING FOR INCOMING SV2 MSG FROM TU\n");
            loop {
                // let message_sv2: EitherFrame = self.upstream_translator.recv_sv2();
                let message_sv2: EitherFrame =
                    self.upstream_translator.receiver.recv().await.unwrap();
                println!("TP RECV SV2 FROM TU: {:?}", &message_sv2);
                let message_sv1: json_rpc::Message = self.parse_sv2_to_sv1(message_sv2);
                self.downstream_translator.send_sv1(message_sv1).await;
            }
        });
    }

    /// Parses a SV1 message and translates to to a SV2 message
    fn parse_sv1_to_sv2(&mut self, _message_sv1: json_rpc::Message) -> EitherFrame {
        // println!("TP RECV SV1 FROM TD TO HANDLE: {:?}", &message_sv1);
        // fn parse_sv1_to_sv2(&mut self, message_sv1: json_rpc::Message) -> () {
        todo!()
        // println!("TP PARSE SV1 -> SV2: {:?}", &message_sv1);
        // ()
    }

    /// Parses a SV2 message and translates to to a SV1 message
    fn parse_sv2_to_sv1(&mut self, message_sv2: EitherFrame) -> json_rpc::Message {
        println!("\n\n\n");
        println!("TP PARSE SV2 -> SV1: {:?}", &message_sv2);
        let mut message: StdFrame = message_sv2.try_into().unwrap();
        let msg_type = message.get_header().unwrap().msg_type();
        let payload = message.payload();
        // let msg_type = message_sv2.get_header().unwrap().msg_type();
        // let payload = message_sv2.payload();
        // println!("\nPAYLOAD: {:?}", &payload);
        // match (msg_type, payload).try_into() {
        //     Ok(Mining::OpenStandardMiningChannelSuccess(m)) => println!("OSMCS: {:?}", m),
        //     Ok(Mining::OpenExtendedMiningChannel(m)) => println!("OSMCS: {:?}", m),
        //     Ok(Mining::NewExtendedMiningJob(m)) => println!("NEMJ: {:?}", m),
        //     Ok(Mining::SetNewPrevHash(m)) => println!("SNPH: {:?}", m),
        //     Ok(m) => println!("OTHER: {:?}", m),
        //     Err(_) => panic!("ERROR"),
        // };
        // let msg_type = message_sv2.get_header();
        // match message_sv2.into() {
        //     Message::Common(m) => println!("OK"),
        //     _ => println!("NOT OK"),
        // };
        // type Message = roles_logic_sv2::parsers::PoolMessages<'static>;
        // type EitherFrame = codec_sv2::decoder::StandardEitherFrame<Message>;

        // let msg_type = message_sv2.into;
        // todo!()
        // match message_sv2 {
        //     Message::NewExtendedMiningJob(m) => println!("\nNEMJ: {:?}\n", m),
        //     Message::SetNewPrevHash(m) => println!("\nSNPH: {:?}\n", m),
        //     _ => println!("\nSOMETHING ELSE\n"),
        //     // Message::Common(m) => println!("\n COMMON MESSAGE: {:?}", m),
        //     // Message::Mining(m) => println!("\n MINING MESSAGE: {:?}", m),
        //     // Message::JobNegotiation(m) => println!("\n JN MESSAGE: {:?}", m),
        //     // Message::TemplateDistribution(m) => println!("\n TD MESSAGE: {:?}", m),
        // }
        let message_str =
            r#"{"params": ["slush.miner1", "password"], "id": 2, "method": "mining.authorize"}"#;
        let message_json: json_rpc::Message = serde_json::from_str(message_str).unwrap();
        println!("\n\n\n");
        message_json
    }
}
