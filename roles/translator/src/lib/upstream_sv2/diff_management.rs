use crate::{
    upstream_sv2::{EitherFrame, Message, StdFrame, Upstream},
    TProxyChannelSendError, TProxyError, TProxyResult,
};
use binary_sv2::u256_from_int;
use roles_logic_sv2::{
    mining_sv2::UpdateChannel, parsers::Mining, utils::Mutex, Error as RolesLogicSv2Error,
};
use std::{sync::Arc, time::Duration};

impl Upstream {
    /// this function checks if the elapsed time since the last update has surpassed the config
    pub(super) async fn try_update_hashrate(self_: Arc<Mutex<Self>>) -> TProxyResult<'static, ()> {
        let (channel_id_option, diff_mgmt, tx_frame) = self_
            .safe_lock(|u| {
                (
                    u.channel_id,
                    u.difficulty_config.clone(),
                    u.connection.sender.clone(),
                )
            })
            .map_err(|_e| TProxyError::PoisonLock)?;
        let channel_id = channel_id_option.ok_or(TProxyError::RolesLogicSv2(
            RolesLogicSv2Error::NotFoundChannelId,
        ))?;
        let (timeout, new_hashrate) = diff_mgmt
            .safe_lock(|d| (d.channel_diff_update_interval, d.channel_nominal_hashrate))
            .map_err(|_e| TProxyError::PoisonLock)?;
        // UPDATE CHANNEL
        let update_channel = UpdateChannel {
            channel_id,
            nominal_hash_rate: new_hashrate,
            maximum_target: u256_from_int(u64::MAX),
        };
        let message = Message::Mining(Mining::UpdateChannel(update_channel));
        let either_frame: StdFrame = message.try_into()?;
        let frame: EitherFrame = either_frame.into();

        tx_frame.send(frame).await.map_err(|e| {
            TProxyError::ChannelErrorSender(TProxyChannelSendError::General(e.to_string()))
        })?;
        async_std::task::sleep(Duration::from_secs(timeout as u64)).await;
        Ok(())
    }
}
