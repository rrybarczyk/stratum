use super::downstream_mining::{DownstreamMiningNode, StdFrame as DownstreamFrame};
use async_channel::{Receiver, SendError, Sender};
use async_recursion::async_recursion;
use async_std::net::TcpStream;
use async_std::task;
use codec_sv2::{Frame, HandshakeRole, Initiator, StandardEitherFrame, StandardSv2Frame};
use messages_sv2::common_messages_sv2::{Protocol, SetupConnection};
use messages_sv2::common_properties::{
    DownstreamChannel, IsMiningDownstream, IsMiningUpstream, IsUpstream, RequestIdMapper,
    StandardChannel, UpstreamChannel,
};
use messages_sv2::errors::Error;
use messages_sv2::handlers::mining::{ChannelType, ParseUpstreamMiningMessages, SendTo};
use messages_sv2::job_dispatcher::GroupChannelJobDispatcher;
use messages_sv2::mining_sv2::*;
use messages_sv2::parsers::{CommonMessages, Mining, MiningDeviceMessages, PoolMessages};
use messages_sv2::routing_logic::{MiningProxyRoutingLogic, MiningRoutingLogic};
use messages_sv2::selectors::{DownstreamMiningSelector, ProxyDownstreamMiningSelector as Prs};
use messages_sv2::utils::{Id, Mutex};
use network_helpers::Connection;
use std::collections::HashMap;
use std::sync::Arc;

pub type Message = PoolMessages<'static>;
pub type StdFrame = StandardSv2Frame<Message>;
pub type EitherFrame = StandardEitherFrame<Message>;
pub type ProxyRemoteSelector = Prs<DownstreamMiningNode>;

/// 1 to 1 connection with a pool
/// Can be either a mining pool or another proxy
/// 1 to 1 connection with an upstream node that implement the mining (sub)protocol can be either a a pool or an
/// upstream proxy.
#[derive(Debug, Clone)]
struct UpstreamMiningConnection {
    receiver: Receiver<EitherFrame>,
    sender: Sender<EitherFrame>,
}

impl UpstreamMiningConnection {
    async fn send(&mut self, sv2_frame: StdFrame) -> Result<(), SendError<EitherFrame>> {
        let either_frame = sv2_frame.into();
        match self.sender.send(either_frame).await {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Sv2MiningConnection {
    version: u16,
    mining_flags: u32,
}

#[derive(Debug)]
pub enum JobDispatcher {
    Group(GroupChannelJobDispatcher),
    None,
}

/// Can be either a mining pool or another proxy
#[derive(Debug)]
pub struct UpstreamMiningNode {
    id: u32,
    job_ids: Arc<Mutex<Id>>,
    total_hash_rate: u64,
    address: SocketAddr,
    //port: u32,
    connection: Option<UpstreamMiningConnection>,
    sv2_connection: Option<Sv2MiningConnection>,
    authority_public_key: [u8; 32],
    /// group_channel id/channel_id -> dispatcher
    pub channel_id_to_job_dispatcher: HashMap<u32, JobDispatcher>,
    /// Each relayd message that have a request_id field must have a unique request_id number
    /// connection-wise.
    /// request_id from downstream is not garanted to be uniquie so must be changed
    request_id_mapper: RequestIdMapper,
    downstream_selector: ProxyRemoteSelector,
}

use crate::MAX_SUPPORTED_VERSION;
use crate::MIN_SUPPORTED_VERSION;
use core::convert::TryInto;
use std::net::SocketAddr;

/// It assume that endpoint NEVER change flags and version! TODO add test for it
impl UpstreamMiningNode {
    pub fn new(
        id: u32,
        address: SocketAddr,
        authority_public_key: [u8; 32],
        job_ids: Arc<Mutex<Id>>,
    ) -> Self {
        let request_id_mapper = RequestIdMapper::new();
        let downstream_selector = ProxyRemoteSelector::new();
        Self {
            id,
            job_ids,
            total_hash_rate: 0,
            address,
            connection: None,
            sv2_connection: None,
            authority_public_key,
            channel_id_to_job_dispatcher: HashMap::new(),
            request_id_mapper,
            downstream_selector,
        }
    }

    /// Try send a message to the upstream node.
    /// If the node is connected and there are no error return Ok(())
    /// If the node is connected and there is an error the message is not sent and an error is
    ///     returned and the upstream is marked as not connected.
    /// If the node is not connected it try to connect and send the message and everything is ok
    ///     the upstream is marked as connected and Ok(()) is returned if not an error is returned.
    ///     TODO verify and test the above statements
    pub async fn send(
        self_mutex: Arc<Mutex<Self>>,
        sv2_frame: StdFrame,
    ) -> Result<(), SendError<EitherFrame>> {
        let (has_sv2_connetcion, mut connection) = self_mutex
            .safe_lock(|self_| (self_.sv2_connection.is_some(), self_.connection.clone()))
            .unwrap();
        //let mut self_ = self_mutex.lock().await;

        match (connection.as_mut(), has_sv2_connetcion) {
            (Some(connection), true) => match connection.send(sv2_frame).await {
                Ok(_) => Ok(()),
                Err(_e) => {
                    Self::connect(self_mutex.clone()).await.unwrap();
                    // It assume that enpoint NEVER change flags and version! TODO add test for
                    // that
                    match Self::setup_connection(self_mutex).await {
                        Ok(()) => Ok(()),
                        Err(()) => panic!(),
                    }
                }
            },
            // It assume that no downstream try to send messages before that the upstream is
            // initialized. This assumption is enforced by the fact that
            // UpstreamMiningNode::pair only pair downstream noder with already
            // initialized upstream nodes! TODO add test for that
            (Some(connection), false) => match connection.send(sv2_frame).await {
                Ok(_) => Ok(()),
                Err(e) => Err(e),
            },
            (None, _) => {
                Self::connect(self_mutex.clone()).await.unwrap();
                let mut connection = self_mutex
                    .safe_lock(|self_| self_.connection.clone())
                    .unwrap();
                match connection.as_mut().unwrap().send(sv2_frame).await {
                    Ok(_) => match Self::setup_connection(self_mutex).await {
                        Ok(()) => Ok(()),
                        Err(()) => panic!(),
                    },
                    Err(e) => {
                        //Self::connect(self_mutex.clone()).await.unwrap();
                        Err(e)
                    }
                }
            }
        }
    }

    async fn receive(self_mutex: Arc<Mutex<Self>>) -> Result<StdFrame, ()> {
        let mut connection = self_mutex
            .safe_lock(|self_| self_.connection.clone())
            .unwrap();
        match connection.as_mut() {
            Some(connection) => match connection.receiver.recv().await {
                Ok(m) => Ok(m.try_into()?),
                Err(_) => {
                    Self::connect(self_mutex).await?;
                    Err(())
                }
            },
            None => todo!("177"),
        }
    }

    async fn connect(self_mutex: Arc<Mutex<Self>>) -> Result<(), ()> {
        let has_connection = self_mutex
            .safe_lock(|self_| self_.connection.is_some())
            .unwrap();
        match has_connection {
            true => Ok(()),
            false => {
                let (address, authority_public_key) = self_mutex
                    .safe_lock(|self_| (self_.address, self_.authority_public_key))
                    .unwrap();
                let socket = TcpStream::connect(address).await.map_err(|_| ())?;
                let initiator = Initiator::from_raw_k(authority_public_key);
                let (receiver, sender) =
                    Connection::new(socket, HandshakeRole::Initiator(initiator)).await;
                let connection = UpstreamMiningConnection { receiver, sender };
                self_mutex
                    .safe_lock(|self_| {
                        self_.connection = Some(connection);
                    })
                    .unwrap();
                Ok(())
            }
        }
    }

    #[async_recursion]
    async fn setup_connection(self_mutex: Arc<Mutex<Self>>) -> Result<(), ()> {
        let sv2_connection = self_mutex.safe_lock(|self_| self_.sv2_connection).unwrap();

        match sv2_connection {
            None => Ok(()),
            Some(sv2_connection) => {
                let flags = sv2_connection.mining_flags;
                let version = sv2_connection.version;
                let frame = self_mutex
                    .safe_lock(|self_| self_.new_setup_connection_frame(flags, version, version))
                    .unwrap();
                Self::send(self_mutex.clone(), frame)
                    .await
                    .map_err(|_| ())?;

                let cloned = self_mutex.clone();
                let mut response = task::spawn(async { Self::receive(cloned).await }).await?;

                let message_type = response.get_header().unwrap().msg_type();
                let payload = response.payload();
                match (message_type, payload).try_into() {
                    Ok(CommonMessages::SetupConnectionSuccess(_)) => {
                        let receiver = self_mutex
                            .safe_lock(|self_| self_.connection.clone().unwrap().receiver)
                            .unwrap();
                        Self::relay_incoming_messages(self_mutex, receiver);
                        Ok(())
                    }
                    _ => panic!(),
                }
            }
        }
    }

    fn relay_incoming_messages(
        self_: Arc<Mutex<Self>>,
        //_downstreams: HashMap<u32, Downstream>,
        receiver: Receiver<EitherFrame>,
    ) {
        task::spawn(async move {
            loop {
                let message = receiver.recv().await.unwrap();
                let incoming: StdFrame = message.try_into().unwrap();
                Self::next(self_.clone(), incoming).await;
            }
        });
    }

    pub async fn next(self_mutex: Arc<Mutex<Self>>, mut incoming: StdFrame) {
        let message_type = incoming.get_header().unwrap().msg_type();
        let payload = incoming.payload();

        let routing_logic = MiningRoutingLogic::Proxy(crate::get_routing_logic());

        let next_message_to_send = UpstreamMiningNode::handle_message_mining(
            self_mutex.clone(),
            message_type,
            payload,
            routing_logic,
        );
        match next_message_to_send {
            Ok(SendTo::RelaySameMessage(downstream)) => {
                let sv2_frame: codec_sv2::Sv2Frame<MiningDeviceMessages, Vec<u8>> =
                    incoming.map(|payload| payload.try_into().unwrap());

                DownstreamMiningNode::send(downstream.clone(), sv2_frame)
                    .await
                    .unwrap();
            }
            Ok(SendTo::RelayNewMessage(downstream_mutex, message)) => {
                let message = MiningDeviceMessages::Mining(message);
                let frame: DownstreamFrame = message.try_into().unwrap();
                DownstreamMiningNode::send(downstream_mutex, frame)
                    .await
                    .unwrap();
            }
            Ok(SendTo::Respond(message)) => {
                let message = PoolMessages::Mining(message);
                let frame: StdFrame = message.try_into().unwrap();
                UpstreamMiningNode::send(self_mutex, frame).await.unwrap();
            }
            Ok(SendTo::Multiple(sends_to)) => {
                for send_to in sends_to {
                    match send_to {
                        SendTo::RelayNewMessage(downstream_mutex, message) => {
                            let message = MiningDeviceMessages::Mining(message);
                            let frame: DownstreamFrame = message.try_into().unwrap();
                            DownstreamMiningNode::send(downstream_mutex, frame)
                                .await
                                .unwrap();
                        }
                        _ => todo!(),
                    }
                }
            }
            Ok(SendTo::None) => (),
            Err(Error::UnexpectedMessage) => todo!("303"),
            Err(_) => todo!("304"),
        }
    }

    #[async_recursion]
    async fn setup_flag_and_version(
        self_mutex: Arc<Mutex<Self>>,
        flags: Option<u32>,
    ) -> Result<(), ()> {
        let flags = flags.unwrap_or(0b0111_0000_0000_0000_0000_0000_0000_0000);
        let min_version = MIN_SUPPORTED_VERSION;
        let max_version = MAX_SUPPORTED_VERSION;
        let frame = self_mutex
            .safe_lock(|self_| self_.new_setup_connection_frame(flags, min_version, max_version))
            .unwrap();
        Self::send(self_mutex.clone(), frame)
            .await
            .map_err(|_| ())?;

        let cloned = self_mutex.clone();
        let mut response = task::spawn(async { Self::receive(cloned).await }).await?;

        let message_type = response.get_header().unwrap().msg_type();
        let payload = response.payload();
        match (message_type, payload).try_into() {
            Ok(CommonMessages::SetupConnectionSuccess(m)) => {
                let receiver = self_mutex
                    .safe_lock(|self_| {
                        self_.sv2_connection = Some(Sv2MiningConnection {
                            version: m.used_version,
                            mining_flags: m.flags,
                        });
                        self_.connection.clone().unwrap().receiver
                    })
                    .unwrap();
                Self::relay_incoming_messages(self_mutex, receiver);
                Ok(())
            }
            Ok(CommonMessages::SetupConnectionError(m)) => {
                if m.flags != 0 {
                    let flags = flags ^ m.flags;
                    // We need to send SetupConnection again as we do not yet know the version of
                    // upstream
                    // TODO debounce this?
                    Self::setup_flag_and_version(self_mutex, Some(flags)).await
                } else {
                    Err(())
                }
            }
            Ok(_) => todo!("356"),
            Err(_) => todo!("357"),
        }
    }

    fn new_setup_connection_frame(
        &self,
        flags: u32,
        min_version: u16,
        max_version: u16,
    ) -> StdFrame {
        let endpoint_host = self
            .address
            .ip()
            .to_string()
            .into_bytes()
            .try_into()
            .unwrap();
        let vendor = String::new().try_into().unwrap();
        let hardware_version = String::new().try_into().unwrap();
        let firmware = String::new().try_into().unwrap();
        let device_id = String::new().try_into().unwrap();
        let setup_connection: PoolMessages = SetupConnection {
            protocol: Protocol::MiningProtocol,
            min_version,
            max_version,
            flags,
            endpoint_host,
            endpoint_port: self.address.port(),
            vendor,
            hardware_version,
            firmware,
            device_id,
        }
        .into();
        setup_connection.try_into().unwrap()
    }

    // Example of how next could be implemented more efficently if no particular good log are
    // needed it just relay the majiority of messages downstream without serializing and
    // deserializing them. In order to find the Downstream at which the message must bu relayed the
    // channel id must be deserialized, but knowing the message type that is a very easy task is
    // either 4 bytes after the header or the first 4 bytes after the header + 4 bytes
    // #[cfg(test)]
    // #[allow(unused)]
    // pub async fn next_faster(&mut self, mut incoming: StdFrame) {
    //     let message_type = incoming.get_header().unwrap().msg_type();

    //     // When a channel is opened we need to setup the channel id in order to relay next messages
    //     // to the right Downstream
    //     if todo!() { // check if message_type is channel related

    //         // When a mining message is received (that is not a channel related message) always relay it downstream
    //     } else if todo!()  { // check if message_type is is a mining message
    //         // everything here can be just relayed downstream

    //         // Other sub(protocol) messages
    //     } else {
    //         todo!()
    //     }
    // }
}

impl
    ParseUpstreamMiningMessages<
        DownstreamMiningNode,
        ProxyRemoteSelector,
        MiningProxyRoutingLogic<DownstreamMiningNode, Self, ProxyRemoteSelector>,
    > for UpstreamMiningNode
{
    fn get_channel_type(&self) -> ChannelType {
        ChannelType::Group
    }

    fn is_work_selection_enabled(&self) -> bool {
        false
    }

    fn handle_open_standard_mining_channel_success(
        &mut self,
        m: OpenStandardMiningChannelSuccess,
        remote: Option<Arc<Mutex<DownstreamMiningNode>>>,
    ) -> Result<SendTo<DownstreamMiningNode>, Error> {
        let down_is_header_only = remote
            .as_ref()
            .unwrap()
            .safe_lock(|remote| remote.is_header_only())
            .unwrap();
        let up_is_header_only = self.is_header_only();
        match (down_is_header_only, up_is_header_only) {
            (true, true) => {
                let channel = DownstreamChannel::Standard(StandardChannel {
                    channel_id: m.channel_id,
                    group_id: m.group_channel_id,
                    target: m.target.into(),
                    extranonce: m.extranonce_prefix.into(),
                });
                remote
                    .as_ref()
                    .unwrap()
                    .safe_lock(|r| r.add_channel(channel))
                    .unwrap();
            }
            (true, false) => {
                let channel = DownstreamChannel::Standard(StandardChannel {
                    channel_id: m.channel_id,
                    group_id: m.group_channel_id,
                    target: m.target.into(),
                    extranonce: m.extranonce_prefix.into(),
                });
                if self
                    .channel_id_to_job_dispatcher
                    .get_mut(&m.group_channel_id)
                    .is_none()
                {
                    let dispatcher = GroupChannelJobDispatcher::new(self.job_ids.clone());
                    self.channel_id_to_job_dispatcher
                        .insert(m.group_channel_id, JobDispatcher::Group(dispatcher));
                }
                remote
                    .as_ref()
                    .unwrap()
                    .safe_lock(|r| r.add_channel(channel))
                    .unwrap();
            }
            (false, true) => {
                todo!()
            }
            (false, false) => {
                let channel = DownstreamChannel::Group(m.group_channel_id);
                remote
                    .as_ref()
                    .unwrap()
                    .safe_lock(|r| r.add_channel(channel))
                    .unwrap();
            }
        }

        Ok(SendTo::RelaySameMessage(remote.unwrap()))
    }

    fn handle_open_extended_mining_channel_success(
        &mut self,
        _m: OpenExtendedMiningChannelSuccess,
    ) -> Result<SendTo<DownstreamMiningNode>, Error> {
        todo!("450")
    }

    fn handle_open_mining_channel_error(
        &mut self,
        _m: OpenMiningChannelError,
    ) -> Result<SendTo<DownstreamMiningNode>, Error> {
        todo!("460")
    }

    fn handle_update_channel_error(
        &mut self,
        _m: UpdateChannelError,
    ) -> Result<SendTo<DownstreamMiningNode>, Error> {
        todo!("470")
    }

    fn handle_close_channel(
        &mut self,
        _m: CloseChannel,
    ) -> Result<SendTo<DownstreamMiningNode>, Error> {
        todo!("480")
    }

    fn handle_set_extranonce_prefix(
        &mut self,
        _m: SetExtranoncePrefix,
    ) -> Result<SendTo<DownstreamMiningNode>, Error> {
        todo!("490")
    }

    fn handle_submit_shares_success(
        &mut self,
        m: SubmitSharesSuccess,
    ) -> Result<SendTo<DownstreamMiningNode>, Error> {
        match &self
            .downstream_selector
            .downstream_from_channel_id(m.channel_id)
        {
            Some(d) => Ok(SendTo::RelaySameMessage(d.clone())),
            None => todo!(),
        }
    }

    fn handle_submit_shares_error(
        &mut self,
        _m: SubmitSharesError,
    ) -> Result<SendTo<DownstreamMiningNode>, Error> {
        todo!("510")
    }

    fn handle_new_mining_job(
        &mut self,
        m: NewMiningJob,
    ) -> Result<SendTo<DownstreamMiningNode>, Error> {
        // One and only one downstream cause the message is not extended
        let downstream = &self
            .downstream_selector
            .get_downstreams_in_channel(m.channel_id)[0];
        crate::add_job_id(m.job_id, self.id);
        Ok(SendTo::RelaySameMessage(downstream.clone()))
    }

    fn handle_new_extended_mining_job(
        &mut self,
        m: NewExtendedMiningJob,
    ) -> Result<SendTo<DownstreamMiningNode>, Error> {
        let id = self.id;
        let downstreams = self
            .downstream_selector
            .get_downstreams_in_channel(m.channel_id);
        let dispacther = self
            .channel_id_to_job_dispatcher
            .get_mut(&m.channel_id)
            .unwrap();
        let mut messages = Vec::with_capacity(downstreams.len());
        for downstream in downstreams {
            downstream
                .safe_lock(|d| {
                    for channel in d.status.get_channels().get_mut(&m.channel_id).unwrap() {
                        match channel {
                            DownstreamChannel::Extended => todo!(),
                            DownstreamChannel::Group(_) => {
                                crate::add_job_id(m.job_id, id);
                                messages.push(SendTo::RelaySameMessage(downstream.clone()))
                            }
                            DownstreamChannel::Standard(channel) => {
                                if let JobDispatcher::Group(d) = dispacther {
                                    let job = d.on_new_extended_mining_job(&m, channel);
                                    crate::add_job_id(job.job_id, id);
                                    let message = Mining::NewMiningJob(job);
                                    messages
                                        .push(SendTo::RelayNewMessage(downstream.clone(), message));
                                } else {
                                    panic!()
                                };
                            }
                        }
                    }
                })
                .unwrap();
        }
        Ok(SendTo::Multiple(messages))
    }

    fn handle_set_new_prev_hash(
        &mut self,
        m: SetNewPrevHash,
    ) -> Result<SendTo<DownstreamMiningNode>, Error> {
        match (
            self.is_header_only(),
            self.channel_id_to_job_dispatcher.get_mut(&m.channel_id),
        ) {
            (true, None) => {
                let downstreams = self
                    .downstream_selector
                    .get_downstreams_in_channel(m.channel_id);
                // If upstream is header only one and only one downstream is in channel
                Ok(SendTo::RelaySameMessage(downstreams[0].clone()))
            }
            (false, Some(JobDispatcher::Group(dispatcher))) => {
                dispatcher.on_new_prev_hash(&m);
                let downstreams = self
                    .downstream_selector
                    .get_downstreams_in_channel(m.channel_id);
                let mut messages: Vec<SendTo<DownstreamMiningNode>> =
                    Vec::with_capacity(downstreams.len());
                for downstream in downstreams {
                    downstream
                        .safe_lock(|d| {
                            for channel in d.status.get_channels().get_mut(&m.channel_id).unwrap() {
                                match channel {
                                    DownstreamChannel::Extended => todo!(),
                                    DownstreamChannel::Group(_) => todo!(),
                                    DownstreamChannel::Standard(channel) => {
                                        let new_prev_hash = SetNewPrevHash {
                                            channel_id: channel.channel_id,
                                            job_id: m.job_id,
                                            prev_hash: m.prev_hash.clone().into_static(),
                                            min_ntime: m.min_ntime,
                                            nbits: m.nbits,
                                        };
                                        let message = Mining::SetNewPrevHash(new_prev_hash);
                                        messages.push(SendTo::RelayNewMessage(
                                            downstream.clone(),
                                            message,
                                        ));
                                    }
                                }
                            }
                        })
                        .unwrap();
                }
                Ok(SendTo::Multiple(messages))
            }
            _ => panic!(),
        }
    }

    fn handle_set_custom_mining_job_success(
        &mut self,
        _m: SetCustomMiningJobSuccess,
    ) -> Result<SendTo<DownstreamMiningNode>, Error> {
        todo!("550")
    }

    fn handle_set_custom_mining_job_error(
        &mut self,
        _m: SetCustomMiningJobError,
    ) -> Result<SendTo<DownstreamMiningNode>, Error> {
        todo!("560")
    }

    fn handle_set_target(&mut self, _m: SetTarget) -> Result<SendTo<DownstreamMiningNode>, Error> {
        todo!("570")
    }

    fn handle_reconnect(&mut self, _m: Reconnect) -> Result<SendTo<DownstreamMiningNode>, Error> {
        todo!("580")
    }

    fn get_request_id_mapper(&mut self) -> Option<Arc<Mutex<RequestIdMapper>>> {
        None
    }
}

pub async fn scan(nodes: Vec<Arc<Mutex<UpstreamMiningNode>>>) {
    let spawn_tasks: Vec<task::JoinHandle<()>> = nodes
        .iter()
        .map(|node| {
            let node = node.clone();
            task::spawn(async move {
                UpstreamMiningNode::setup_flag_and_version(node, None)
                    .await
                    .unwrap();
            })
        })
        .collect();
    for task in spawn_tasks {
        task.await
    }
}

impl IsUpstream<DownstreamMiningNode, ProxyRemoteSelector> for UpstreamMiningNode {
    fn get_version(&self) -> u16 {
        self.sv2_connection.unwrap().version
    }

    fn get_flags(&self) -> u32 {
        self.sv2_connection.unwrap().mining_flags
    }

    fn get_supported_protocols(&self) -> Vec<Protocol> {
        vec![Protocol::MiningProtocol]
    }

    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_mapper(&mut self) -> Option<&mut RequestIdMapper> {
        Some(&mut self.request_id_mapper)
    }

    fn get_remote_selector(&mut self) -> &mut ProxyRemoteSelector {
        &mut self.downstream_selector
    }
}
impl IsMiningUpstream<DownstreamMiningNode, ProxyRemoteSelector> for UpstreamMiningNode {
    fn total_hash_rate(&self) -> u64 {
        self.total_hash_rate
    }
    fn add_hash_rate(&mut self, to_add: u64) {
        self.total_hash_rate += to_add;
    }
    fn get_opened_channels(&mut self) -> &mut Vec<UpstreamChannel> {
        todo!()
    }
    fn update_channels(&mut self, _channel: UpstreamChannel) {
        todo!()
    }
}
