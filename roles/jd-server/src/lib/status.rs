use roles_logic_sv2::parsers::Mining;

use super::Error;

/// Each sending side of the status channel
/// should be wrapped with this enum to allow
/// the main thread to know which component sent the message
#[derive(Debug)]
pub enum Sender {
    Downstream(async_channel::Sender<Status>),
    DownstreamListener(async_channel::Sender<Status>),
    Upstream(async_channel::Sender<Status>),
}

impl Clone for Sender {
    fn clone(&self) -> Self {
        match self {
            Self::Downstream(inner) => Self::Downstream(inner.clone()),
            Self::DownstreamListener(inner) => Self::DownstreamListener(inner.clone()),
            Self::Upstream(inner) => Self::Upstream(inner.clone()),
        }
    }
}

#[derive(Debug)]
pub enum State {
    DownstreamShutdown(Error),
    TemplateProviderShutdown(Error),
    DownstreamInstanceDropped(u32),
    Healthy(String),
}

/// message to be sent to the status loop on the main thread
#[derive(Debug)]
pub struct Status {
    pub state: State,
}

/// this function is used to discern which componnent experienced the event.
/// With this knowledge we can wrap the status message with information (`State` variants) so
/// the main status loop can decide what should happen
async fn send_status(
    sender: &Sender,
    e: Error,
    outcome: error_handling::ErrorBranch,
) -> error_handling::ErrorBranch {
    match sender {
        Sender::Downstream(tx) => match e {
            Error::Sv2ProtocolError((id, Mining::OpenMiningChannelError(_))) => {
                tx.send(Status {
                    state: State::DownstreamInstanceDropped(id),
                })
                .await
                .unwrap_or(());
            }
            Error::ChannelRecv(_) => {
                tx.send(Status {
                    state: State::DownstreamShutdown(e),
                })
                .await
                .unwrap_or(());
            }
            Error::MempoolError(_) => {
                tx.send(Status {
                    state: State::TemplateProviderShutdown(e),
                })
                .await
                .unwrap_or(());
            }
            _ => {
                let string_err = e.to_string();
                tx.send(Status {
                    state: State::Healthy(string_err),
                })
                .await
                .unwrap_or(());
            }
        },
        Sender::DownstreamListener(tx) => {
            tx.send(Status {
                state: State::DownstreamShutdown(e),
            })
            .await
            .unwrap_or(());
        }
        Sender::Upstream(tx) => {
            tx.send(Status {
                state: State::TemplateProviderShutdown(e),
            })
            .await
            .unwrap_or(());
        }
    }
    outcome
}

// this is called by `error_handling::handle_result!`
pub async fn handle_error(sender: &Sender, e: Error) -> error_handling::ErrorBranch {
    tracing::debug!("Error: {:?}", &e);
    match e {
        Error::ConfigError(_) => send_status(sender, e, error_handling::ErrorBranch::Break).await,
        Error::Io(_) => send_status(sender, e, error_handling::ErrorBranch::Break).await,
        Error::ChannelSend(_) => {
            //This should be a continue because if we fail to send to 1 downstream we should continue
            //processing the other downstreams in the loop we are in. Otherwise if a downstream fails
            //to send to then subsequent downstreams in the map won't get send called on them
            send_status(sender, e, error_handling::ErrorBranch::Continue).await
        }
        Error::ChannelRecv(_) => send_status(sender, e, error_handling::ErrorBranch::Break).await,
        Error::BinarySv2(_) => send_status(sender, e, error_handling::ErrorBranch::Break).await,
        Error::Codec(_) => send_status(sender, e, error_handling::ErrorBranch::Break).await,
        Error::Noise(_) => send_status(sender, e, error_handling::ErrorBranch::Continue).await,
        Error::RolesLogic(_) => send_status(sender, e, error_handling::ErrorBranch::Break).await,
        Error::Custom(_) => send_status(sender, e, error_handling::ErrorBranch::Break).await,
        Error::Framing(_) => send_status(sender, e, error_handling::ErrorBranch::Break).await,
        Error::PoisonLock(_) => send_status(sender, e, error_handling::ErrorBranch::Break).await,
        Error::Sv2ProtocolError(_) => {
            send_status(sender, e, error_handling::ErrorBranch::Break).await
        }
        Error::MempoolError(_) => send_status(sender, e, error_handling::ErrorBranch::Break).await,
        Error::ImpossibleToReconstructBlock(_) => {
            send_status(sender, e, error_handling::ErrorBranch::Continue).await
        }
        Error::NoLastDeclaredJob => {
            send_status(sender, e, error_handling::ErrorBranch::Continue).await
        }
    }
}
