use crate::{EitherFrame, StdFrame};
use async_channel::{Receiver, Sender};
use codec_sv2::Frame;
use roles_logic_sv2::{
    common_messages_sv2::{
        has_requires_std_job, has_version_rolling, has_work_selection, SetupConnection,
        SetupConnectionSuccess,
    },
    common_properties::CommonDownstreamData,
    errors::Error,
    handlers::common::ParseDownstreamCommonMessages,
    parsers::{CommonMessages, PoolMessages},
    routing_logic::{CommonRoutingLogic, NoRouting},
    utils::Mutex,
};
use std::{convert::TryInto, sync::Arc};

pub struct SetupConnectionHandler {
    header_only: Option<bool>,
}

impl SetupConnectionHandler {
    pub fn new() -> Self {
        Self { header_only: None }
    }
    pub async fn setup(
        self_: Arc<Mutex<Self>>,
        receiver: &mut Receiver<EitherFrame>,
        sender: &mut Sender<EitherFrame>,
    ) -> Result<CommonDownstreamData, ()> {
        let mut incoming: StdFrame = receiver.recv().await.unwrap().try_into().unwrap();
        let message_type = incoming.get_header().unwrap().msg_type();
        let payload = incoming.payload();
        let response = ParseDownstreamCommonMessages::handle_message_common(
            self_.clone(),
            message_type,
            payload,
            CommonRoutingLogic::None,
        )
        .unwrap();

        let message = response.into_message().unwrap();

        let sv2_frame: StdFrame = PoolMessages::Common(message.clone()).try_into().unwrap();
        let sv2_frame = sv2_frame.into();
        sender.send(sv2_frame).await.unwrap();
        self_.safe_lock(|s| s.header_only.unwrap()).unwrap();

        match message {
            CommonMessages::SetupConnectionSuccess(m) => Ok(CommonDownstreamData {
                header_only: has_requires_std_job(m.flags),
                work_selection: has_work_selection(m.flags),
                version_rolling: has_version_rolling(m.flags),
            }),
            _ => panic!(),
        }
    }
}

impl ParseDownstreamCommonMessages<NoRouting> for SetupConnectionHandler {
    fn handle_setup_connection(
        &mut self,
        incoming: SetupConnection,
        _: Option<Result<(CommonDownstreamData, SetupConnectionSuccess), Error>>,
    ) -> Result<roles_logic_sv2::handlers::common::SendTo, Error> {
        use roles_logic_sv2::handlers::common::SendTo;
        let header_only = incoming.requires_standard_job();
        self.header_only = Some(header_only);
        Ok(SendTo::RelayNewMessage(
            CommonMessages::SetupConnectionSuccess(SetupConnectionSuccess {
                flags: 0,
                used_version: 2,
            }),
        ))
    }
}
