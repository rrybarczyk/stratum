#[cfg(not(feature = "with_serde"))]
use alloc::vec::Vec;
#[cfg(not(feature = "with_serde"))]
use binary_sv2::binary_codec_sv2;
use binary_sv2::{Deserialize, Serialize, B032};

/// # SetExtranoncePrefix (Server -> Client)
///
/// Changes downstream node’s extranonce prefix. It is applicable for all jobs sent after this
/// message on a given channel (both jobs provided by the upstream or jobs introduced by
/// SetCustomMiningJob message). This message is applicable only for explicitly opened
/// extended channels or standard channels (not group channels).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SetExtranoncePrefix<'decoder> {
    /// Extended or standard channel identifier.
    pub channel_id: u32,
    /// Bytes used as implicit first part of extranonce.
    #[cfg_attr(feature = "with_serde", serde(borrow))]
    pub extranonce_prefix: B032<'decoder>,
}
