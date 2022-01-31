//! Traits that implements very basic properties that every implementation should implements
use crate::selectors::{
    DownstreamMiningSelector, DownstreamSelector, NullDownstreamMiningSelector,
};
use common_messages_sv2::has_requires_std_job;
use common_messages_sv2::{Protocol, SetupConnection};
use std::collections::HashMap;
use std::fmt::Debug as D;

/// Defines a downstream mining node in its simplest form.
#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub struct CommonDownstreamData {
    pub id: u32,
    pub header_only: bool,
    pub work_selection: bool,
    pub version_rolling: bool,
}

/// SetupConnection sugared
#[derive(Debug, Copy, Clone)]
pub struct PairSettings {
    pub protocol: Protocol,
    pub min_v: u16,
    pub max_v: u16,
    pub flags: u32,
}

pub trait IsUpstream<Down: IsDownstream, Sel: DownstreamSelector<Down> + ?Sized> {
    fn get_version(&self) -> u16;
    fn get_flags(&self) -> u32;
    fn get_supported_protocols(&self) -> Vec<Protocol>;
    fn is_pairable(&self, pair_settings: &PairSettings) -> bool {
        let protocol = pair_settings.protocol;
        let min_v = pair_settings.min_v;
        let max_v = pair_settings.max_v;
        let flags = pair_settings.flags;

        let check_version = self.get_version() >= min_v && self.get_version() <= max_v;
        let check_flags = SetupConnection::check_flags(protocol, flags, self.get_flags());
        check_version && check_flags
    }
    fn get_id(&self) -> u32;
    fn get_mapper(&mut self) -> Option<&mut RequestIdMapper>;
    fn get_remote_selector(&mut self) -> &mut Sel;
}

/// Channel opened with upsrtream
#[derive(Debug, Clone, Copy)]
pub enum UpstreamChannel {
    // nominal hash rate
    Standard(f32),
    Group,
    Extended,
}

/// Channel opened with downstream
#[derive(Debug, Clone)]
pub enum DownstreamChannel {
    // channel id, target, extranonce prefix, group channel id
    Standard(StandardChannel),
    Group(u32),
    Extended,
}

impl DownstreamChannel {
    pub fn group_id(&self) -> u32 {
        match self {
            DownstreamChannel::Standard(s) => s.group_id,
            DownstreamChannel::Group(id) => *id,
            DownstreamChannel::Extended => todo!(),
        }
    }
    pub fn channel_id(&self) -> u32 {
        match self {
            DownstreamChannel::Standard(s) => s.channel_id,
            DownstreamChannel::Group(id) => *id,
            DownstreamChannel::Extended => todo!(),
        }
    }
}
use mining_sv2::{Extranonce, Target};

#[derive(Debug, Clone)]
pub struct StandardChannel {
    pub channel_id: u32,
    pub group_id: u32,
    pub target: Target,
    pub extranonce: Extranonce,
}

/// General properties that each mining upstream node that implement the SV2 protocol should have.
pub trait IsMiningUpstream<Down: IsMiningDownstream, Sel: DownstreamMiningSelector<Down> + ?Sized>:
    IsUpstream<Down, Sel>
{
    fn total_hash_rate(&self) -> u64;
    fn add_hash_rate(&mut self, to_add: u64);
    fn get_opened_channels(&mut self) -> &mut Vec<UpstreamChannel>;
    fn update_channels(&mut self, c: UpstreamChannel);
    fn is_header_only(&self) -> bool {
        has_requires_std_job(self.get_flags())
    }
}

/// General properties that each mining downstream node that implements the SV2 protocol should
/// have.
pub trait IsDownstream {
    fn get_downstream_mining_data(&self) -> CommonDownstreamData;
}

/// General properties that each mining upstream node that implement the SV2 protocol should have.
pub trait IsMiningDownstream: IsDownstream {
    fn is_header_only(&self) -> bool {
        self.get_downstream_mining_data().header_only
    }
}

/// Implemented for the NullDownstreamMiningSelector
impl<Down: IsDownstream + D> IsUpstream<Down, NullDownstreamMiningSelector> for () {
    fn get_version(&self) -> u16 {
        unreachable!("0");
    }

    fn get_flags(&self) -> u32 {
        unreachable!("1");
    }

    fn get_supported_protocols(&self) -> Vec<Protocol> {
        unreachable!("2");
    }
    fn get_id(&self) -> u32 {
        unreachable!("b");
    }

    fn get_mapper(&mut self) -> Option<&mut RequestIdMapper> {
        todo!()
    }

    fn get_remote_selector(&mut self) -> &mut NullDownstreamMiningSelector {
        todo!()
    }
}

impl<Down: IsMiningDownstream + D> IsMiningUpstream<Down, NullDownstreamMiningSelector> for () {
    fn total_hash_rate(&self) -> u64 {
        todo!()
    }

    fn add_hash_rate(&mut self, _to_add: u64) {
        todo!()
    }
    fn get_opened_channels(&mut self) -> &mut Vec<UpstreamChannel> {
        todo!()
    }

    fn update_channels(&mut self, _: UpstreamChannel) {
        todo!()
    }
}

/// Implemented for the NullDownstreamMiningSelector
impl IsDownstream for () {
    fn get_downstream_mining_data(&self) -> CommonDownstreamData {
        unreachable!("c");
    }
}

impl IsMiningDownstream for () {}

/// Proxies likely need to change the request ids of downstream's messages. They also need to
/// remember the original id to patch the upstream's response with it.
#[derive(Debug, Default, PartialEq)]
pub struct RequestIdMapper {
    /// Stores the client-specified request ids in a hash map. The first entry is the
    /// current request id, the second entry is the previous request id.
    // upstream id -> downstream id, RRQ: is my explanation on the above line correct?
    request_ids_map: HashMap<u32, u32>,
    /// The next request id that will be assigned.
    next_id: u32,
}

impl RequestIdMapper {
    /// Instantiate a new RequestIdMapper initialized with an empty hash map and 0 for the next
    /// request id (will be incremented when `RequestIdMapper::on_open_channel` is called).
    pub fn new() -> Self {
        Self {
            request_ids_map: HashMap::new(),
            next_id: 0,
        }
    }

    /// Increments the request id and inserts this new incremented id along with the old id in a
    /// hash map.
    pub fn on_open_channel(&mut self, id: u32) -> u32 {
        let new_id = self.next_id;
        self.next_id += 1;

        //let mut inner = self.request_ids_map.lock().unwrap();
        self.request_ids_map.insert(new_id, id);
        new_id
    }

    /// Removes the specified request id from hash map.
    pub fn remove(&mut self, upstream_id: u32) -> u32 {
        //let mut inner = self.request_ids_map.lock().unwrap();
        self.request_ids_map.remove(&upstream_id).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_new_request_id_mapper_struct() {
        let actual = RequestIdMapper::new();
        let expect = RequestIdMapper {
            request_ids_map: HashMap::new(),
            next_id: 0,
        };
        assert_eq!(actual, expect);
    }

    #[test]
    fn inserts_new_id_on_open_channel() {
        let id = 0;
        let mut request_id_mapper = RequestIdMapper {
            request_ids_map: HashMap::new(),
            next_id: id,
        };
        let actual = request_id_mapper.on_open_channel(0);
        let mut request_ids_map_expect = HashMap::new();

        request_ids_map_expect.insert(id + 1, id);

        let expect = 0;
        assert_eq!(actual, expect);
    }
}
