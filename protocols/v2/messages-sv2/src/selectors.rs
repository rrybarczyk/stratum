//! Selectors are used from the routing logic in order to chose to which remote or set of remotes
//! a message should be relayed, or to which remote or set of remotes a message should be sent.
use crate::common_properties::{IsDownstream, IsMiningDownstream, IsMiningUpstream, PairSettings};
use crate::errors::Error;
use crate::utils::Mutex;
use std::collections::HashMap;
use std::fmt::Debug as D;
use std::sync::Arc;

/// A DownstreamMiningSelector useful for routing messages in a mining proxy
#[derive(Debug, Clone, Default)]
pub struct ProxyDownstreamMiningSelector<Down: IsDownstream> {
    request_id_to_remotes: HashMap<u32, Arc<Mutex<Down>>>,
    channel_id_to_downstreams: HashMap<u32, Vec<Arc<Mutex<Down>>>>,
    channel_id_to_downstream: HashMap<u32, Arc<Mutex<Down>>>,
}

impl<Down: IsDownstream> ProxyDownstreamMiningSelector<Down> {
    pub fn new() -> Self {
        Self {
            request_id_to_remotes: HashMap::new(),
            channel_id_to_downstreams: HashMap::new(),
            channel_id_to_downstream: HashMap::new(),
        }
    }
    pub fn new_as_mutex() -> Arc<Mutex<Self>>
    where
        Self: Sized,
    {
        Arc::new(Mutex::new(Self::new()))
    }
}

impl<Down: IsMiningDownstream> DownstreamMiningSelector<Down>
    for ProxyDownstreamMiningSelector<Down>
{
    fn on_open_standard_channel_request(&mut self, request_id: u32, downstream: Arc<Mutex<Down>>) {
        self.request_id_to_remotes.insert(request_id, downstream);
    }

    fn on_open_standard_channel_success(
        &mut self,
        request_id: u32,
        g_channel_id: u32,
        channel_id: u32,
    ) -> Arc<Mutex<Down>> {
        let downstream = self.request_id_to_remotes.remove(&request_id).unwrap();
        self.channel_id_to_downstream
            .insert(channel_id, downstream.clone());
        match self.channel_id_to_downstreams.get_mut(&g_channel_id) {
            None => {
                self.channel_id_to_downstreams
                    .insert(g_channel_id, vec![downstream.clone()]);
            }
            Some(x) => x.push(downstream.clone()),
        }
        downstream
    }

    fn get_downstreams_in_channel(&self, channel_id: u32) -> &Vec<Arc<Mutex<Down>>> {
        self.channel_id_to_downstreams.get(&channel_id).unwrap()
    }

    fn remote_from_request_id(&mut self, _request_id: u32) -> Arc<Mutex<Down>> {
        todo!()
    }

    fn downstream_from_channel_id(&self, channel_id: u32) -> Option<Arc<Mutex<Down>>> {
        self.channel_id_to_downstream.get(&channel_id).cloned()
    }
}

impl<Down: IsMiningDownstream> DownstreamSelector<Down> for ProxyDownstreamMiningSelector<Down> {}

/// Implemented by a selector used by an upstream mining node or an upstream mining node
/// abstraction in order to find the right downstream to which a message should be sent or relayed
pub trait DownstreamMiningSelector<Downstream: IsMiningDownstream>:
    DownstreamSelector<Downstream>
{
    fn on_open_standard_channel_request(
        &mut self,
        request_id: u32,
        downstream: Arc<Mutex<Downstream>>,
    );

    fn on_open_standard_channel_success(
        &mut self,
        request_id: u32,
        g_channel_id: u32,
        channel_id: u32,
    ) -> Arc<Mutex<Downstream>>;

    // group / standard naming is terrible channel_id in this case can be  either the channel_id
    // or the group_channel_id
    fn get_downstreams_in_channel(&self, channel_id: u32) -> &Vec<Arc<Mutex<Downstream>>>;

    // only for standard
    fn downstream_from_channel_id(&self, channel_id: u32) -> Option<Arc<Mutex<Downstream>>>;

    fn remote_from_request_id(&mut self, request_id: u32) -> Arc<Mutex<Downstream>>;
}

pub trait DownstreamSelector<D: IsDownstream> {}

/// A DownstreamMiningSelector that do nothing. Useful when ParseDownstreamCommonMessages or
/// ParseUpstreamCommonMessages must be implemented in very simple application (eg for test
/// puorposes)
#[derive(Debug, Clone, Copy, Default)]
pub struct NullDownstreamMiningSelector();

impl NullDownstreamMiningSelector {
    pub fn new() -> Self {
        NullDownstreamMiningSelector()
    }
    pub fn new_as_mutex() -> Arc<Mutex<Self>>
    where
        Self: Sized,
    {
        Arc::new(Mutex::new(Self::new()))
    }
}

impl<Down: IsMiningDownstream + D> DownstreamMiningSelector<Down> for NullDownstreamMiningSelector {
    fn on_open_standard_channel_request(
        &mut self,
        _request_id: u32,
        _downstream: Arc<Mutex<Down>>,
    ) {
        unreachable!("on_open_standard_channel_request")
    }

    fn on_open_standard_channel_success(
        &mut self,
        _request_id: u32,
        _channel_id: u32,
        _channel_id_2: u32,
    ) -> Arc<Mutex<Down>> {
        unreachable!("on_open_standard_channel_success")
    }

    fn get_downstreams_in_channel(&self, _channel_id: u32) -> &Vec<Arc<Mutex<Down>>> {
        unreachable!("get_downstreams_in_channel")
    }

    fn remote_from_request_id(&mut self, _request_id: u32) -> Arc<Mutex<Down>> {
        unreachable!("remote_from_request_id")
    }

    fn downstream_from_channel_id(&self, _channel_id: u32) -> Option<Arc<Mutex<Down>>> {
        unreachable!("downstream_from_channel_id")
    }
}

impl<Down: IsDownstream + D> DownstreamSelector<Down> for NullDownstreamMiningSelector {}

pub trait UpstreamSelector {}

pub trait UpstreamMiningSelctor<
    Down: IsMiningDownstream,
    Up: IsMiningUpstream<Down, Sel>,
    Sel: DownstreamMiningSelector<Down>,
>: UpstreamSelector
{
    #[allow(clippy::type_complexity)]
    fn on_setup_connection(
        &mut self,
        pair_settings: &PairSettings,
    ) -> Result<(Vec<Arc<Mutex<Up>>>, u32), Error>;
    fn get_upstream(&self, upstream_id: u32) -> Option<Arc<Mutex<Up>>>;
}

/// From a set of known mining nodes, the upstream selector chooses which one(s) are configured to
/// accept messages from a specific mining downstream node.
#[derive(Debug)]
pub struct GeneralMiningSelector<
    Sel: DownstreamMiningSelector<Down>,
    Down: IsMiningDownstream,
    Up: IsMiningUpstream<Down, Sel>,
> {
    upstreams: Vec<Arc<Mutex<Up>>>,
    id_to_upstream: HashMap<u32, Arc<Mutex<Up>>>,
    sel: std::marker::PhantomData<Sel>,
    down: std::marker::PhantomData<Down>,
}

impl<
        Sel: DownstreamMiningSelector<Down>,
        Up: IsMiningUpstream<Down, Sel>,
        Down: IsMiningDownstream,
    > GeneralMiningSelector<Sel, Down, Up>
{
    pub fn new(upstreams: Vec<Arc<Mutex<Up>>>) -> Self {
        let mut id_to_upstream = HashMap::new();
        for up in &upstreams {
            id_to_upstream.insert(up.safe_lock(|u| u.get_id()).unwrap(), up.clone());
        }
        Self {
            upstreams,
            id_to_upstream,
            sel: std::marker::PhantomData,
            down: std::marker::PhantomData,
        }
    }
}
impl<
        Sel: DownstreamMiningSelector<Down>,
        Down: IsMiningDownstream,
        Up: IsMiningUpstream<Down, Sel>,
    > UpstreamSelector for GeneralMiningSelector<Sel, Down, Up>
{
}

impl<
        Sel: DownstreamMiningSelector<Down>,
        Down: IsMiningDownstream,
        Up: IsMiningUpstream<Down, Sel>,
    > UpstreamMiningSelctor<Down, Up, Sel> for GeneralMiningSelector<Sel, Down, Up>
{
    /// Returns the set of mining upstream nodes that can accept messages from a downstream node
    /// with the passed `PairSettings` and the sum of all the accepted flags.
    #[allow(clippy::type_complexity)]
    fn on_setup_connection(
        &mut self,
        pair_settings: &PairSettings,
    ) -> Result<(Vec<Arc<Mutex<Up>>>, u32), Error> {
        let mut supported_upstreams = vec![];
        let mut supported_flags: u32 = 0;
        for node in &self.upstreams {
            let is_pairable = node
                .safe_lock(|node| node.is_pairable(pair_settings))
                .unwrap();
            if is_pairable {
                // equal to supported_flags = supported_flags | node.safe_lock(|n| n.get_flags()).unwrap()
                supported_flags |= node.safe_lock(|n| n.get_flags()).unwrap();
                supported_upstreams.push(node.clone());
            }
        }
        if !supported_upstreams.is_empty() {
            return Ok((supported_upstreams, supported_flags));
        }

        // TODO should return something more meaningfull
        Err(Error::NoPairableUpstream((2, 2, 0)))
    }

    /// Returns the upstream id.
    fn get_upstream(&self, upstream_id: u32) -> Option<Arc<Mutex<Up>>> {
        self.id_to_upstream.get(&upstream_id).cloned()
    }
}
