use async_channel::{bounded, Receiver, Sender};
use binary_sv2::{Deserialize, Serialize};
use core::convert::TryInto;
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex,
    task,
};

use binary_sv2::GetSize;
use codec_sv2::{
    Frame, HandShakeFrame, HandshakeRole, Initiator, Responder, StandardEitherFrame,
    StandardNoiseDecoder,
};

#[derive(Debug)]
pub struct Connection {
    /// Noise protocol state
    pub state: codec_sv2::State,
}

impl Connection {
    #[allow(clippy::new_ret_no_self)]
    pub async fn new<'a, Message: Serialize + Deserialize<'a> + GetSize + Send + 'static>(
        stream: TcpStream,
        role: HandshakeRole,
    ) -> (
        Receiver<StandardEitherFrame<Message>>,
        Sender<StandardEitherFrame<Message>>,
    ) {
        let (mut reader, mut writer) = stream.into_split();

        let (sender_incoming, receiver_incoming): (
            Sender<StandardEitherFrame<Message>>,
            Receiver<StandardEitherFrame<Message>>,
        ) = bounded(10); // TODO caller should provide this param
        let (sender_outgoing, receiver_outgoing): (
            Sender<StandardEitherFrame<Message>>,
            Receiver<StandardEitherFrame<Message>>,
        ) = bounded(10); // TODO caller should provide this param

        // Set noise protocol state to `NotInitialized`
        let state = codec_sv2::State::new();

        let connection = Arc::new(Mutex::new(Self { state }));

        let cloned1 = connection.clone();
        let cloned2 = connection.clone();

        // RECEIVE AND PARSE INCOMING MESSAGES FROM TCP STREAM
        task::spawn(async move {
            let mut decoder = StandardNoiseDecoder::<Message>::new();

            loop {
                let writable = decoder.writable();
                match reader.read_exact(writable).await {
                    Ok(_) => {
                        let mut connection = cloned1.lock().await;

                        if let Ok(x) = decoder.next_frame(&mut connection.state) {
                            sender_incoming.send(x).await.unwrap();
                        }
                    }
                    Err(_) => {
                        // Just fail and force to reinitialize everything
                        panic!()
                    }
                }
            }
        });

        let receiver_outgoing_cloned = receiver_outgoing.clone();

        // ENCODE AND SEND INCOMING MESSAGES TO TCP STREAM
        task::spawn(async move {
            let mut encoder = codec_sv2::NoiseEncoder::<Message>::new();

            loop {
                let received = receiver_outgoing.recv().await;
                match received {
                    Ok(frame) => {
                        let mut connection = cloned2.lock().await;
                        let b = encoder.encode(frame, &mut connection.state).unwrap();
                        let b = b.as_ref();

                        match (&mut writer).write_all(b).await {
                            Ok(_) => (),
                            Err(_) => {
                                let _ = writer.shutdown().await;
                                // Just fail and force to reinitialize everything
                                panic!()
                            }
                        }
                    }
                    Err(_) => {
                        // Just fail and force to reinitilize everything
                        let _ = writer.shutdown().await;
                        panic!()
                    }
                };
            }
        });

        // DO THE NOISE HANDSHAKE
        let transport_mode = match role {
            HandshakeRole::Initiator(_) => {
                Self::initialize_as_downstream(
                    role,
                    sender_outgoing.clone(),
                    receiver_incoming.clone(),
                )
                .await
            }
            HandshakeRole::Responder(_) => {
                Self::initialize_as_upstream(
                    role,
                    sender_outgoing.clone(),
                    receiver_outgoing_cloned,
                    receiver_incoming.clone(),
                )
                .await
            }
        };

        Self::set_state(connection.clone(), transport_mode).await;

        (receiver_incoming, sender_outgoing)
    }

    async fn set_state(self_: Arc<Mutex<Self>>, state: codec_sv2::State) {
        loop {
            if let Ok(mut connection) = self_.try_lock() {
                connection.state = state;
                break;
            };
        }
    }

    async fn initialize_as_downstream<'a, Message: Serialize + Deserialize<'a> + GetSize>(
        role: HandshakeRole,
        sender_outgoing: Sender<StandardEitherFrame<Message>>,
        receiver_incoming: Receiver<StandardEitherFrame<Message>>,
    ) -> codec_sv2::State {
        // Set state handshake mode, where `codec` is negotiating the keys
        let mut state = codec_sv2::State::initialize(role);

        // Downstream (`Initiator`) takes the first handshake step.
        // Upstream (`Responder`) sends an `ExpectReply` message to the Downstream (`Initiator`)
        // containing their supported encryption algorithms
        let first_message = state.step(None).unwrap();
        sender_outgoing.send(first_message.into()).await.unwrap();

        // Upstream receives an `ExpectReply` message from the Downstream containing the selected
        // encryption algorithm
        let second_message = receiver_incoming.recv().await.unwrap();
        let mut second_message: HandShakeFrame = second_message.try_into().unwrap();
        let second_message = second_message.payload().to_vec();

        // Downstream updates the handshake state with the chosen encryption algorithm and sends an
        // `ExpectReply` message containing their ephemeral public key to the Upstream
        let third_message = state.step(Some(second_message)).unwrap();
        sender_outgoing.send(third_message.into()).await.unwrap();

        // Downstream receives a `NoMoreReply` messages from the Upstream containing:
        // e: `Initiator`'s ephemeral public key
        // ee: `Responder`'s ephemeral public key
        // s: `Initiator`'s static public key
        // es: Token indicates a DH between the `Initiator`'s ephemeral public key and the
        //     `Responder`'s static public key
        // SIGNATURE_NOISE_MESSAGE: encrypted noise message
        let fourth_message = receiver_incoming.recv().await.unwrap();
        let mut fourth_message: HandShakeFrame = fourth_message.try_into().unwrap();
        let fourth_message = fourth_message.payload().to_vec();
        dbg!(&fourth_message);

        state
            .step(Some(fourth_message))
            .expect("Error on fourth message step");

        state.into_transport_mode().unwrap()
    }

    async fn initialize_as_upstream<'a, Message: Serialize + Deserialize<'a> + GetSize>(
        role: HandshakeRole,
        sender_outgoing: Sender<StandardEitherFrame<Message>>,
        sender_incoming: Receiver<StandardEitherFrame<Message>>,
        receiver_incoming: Receiver<StandardEitherFrame<Message>>,
    ) -> codec_sv2::State {
        let mut state = codec_sv2::State::initialize(role);

        // Upstream (`Responder`) receives an `ExpectReply` message from the Downstream
        // (`Initiator`) containing their support encryption algorithms
        let mut first_message: HandShakeFrame =
            receiver_incoming.recv().await.unwrap().try_into().unwrap();
        let first_message = first_message.payload().to_vec();

        // Upstream sends an `ExpectReply` message to the Downstream with the selected encryption
        // algorithm
        let second_message = state.step(Some(first_message)).unwrap();
        sender_outgoing.send(second_message.into()).await.unwrap();

        // Upstream receives an `ExpectReply` message from the Downstream containing their
        // ephemeral public key (e)
        let mut third_message: HandShakeFrame =
            receiver_incoming.recv().await.unwrap().try_into().unwrap();
        let third_message = third_message.payload().to_vec();

        // Upstream creates a `NoMoreReply` message and sends to the Downstream.
        // This messages contains:
        // e: Downstream's ephemeral public key
        // ee: Upstream's ephemeral public key
        // s: Downstream's static public key
        // es: Token indicates a DH between the Downstream's ephemeral public key and the
        //     Upstream's static public key
        // The Downstream verifies the Upstream's signatures of the remote static key and creates a
        // `Done` reply message indicating the handshake is complete
        let fourth_message = state.step(Some(third_message)).unwrap();
        sender_outgoing.send(fourth_message.into()).await.unwrap();

        // Every 1 ms, check if fourth message has been sent from the Downstream to the Upstream
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            if sender_incoming.is_empty() {
                break;
            }
        }

        state.into_transport_mode().unwrap()
    }
}

pub async fn listen(
    address: &str,
    authority_public_key: [u8; 32],
    authority_private_key: [u8; 32],
    cert_validity: Duration,
    sender: Sender<(TcpStream, HandshakeRole)>,
) {
    let listner = TcpListener::bind(address).await.unwrap();
    loop {
        if let Ok((stream, _)) = listner.accept().await {
            let responder = Responder::from_authority_kp(
                &authority_public_key[..],
                &authority_private_key[..],
                cert_validity,
            )
            .unwrap();
            let role = HandshakeRole::Responder(responder);
            let _ = sender.send((stream, role)).await;
        }
    }
}

pub async fn connect(
    address: &str,
    authority_public_key: [u8; 32],
) -> Result<(TcpStream, HandshakeRole), ()> {
    let stream = TcpStream::connect(address).await.map_err(|_| ())?;
    let initiator = Initiator::from_raw_k(authority_public_key).unwrap();
    let role = HandshakeRole::Initiator(initiator);
    Ok((stream, role))
}
