use crate::mutex::Mutex;
use std::sync::Arc;

/// Message is a serializable entity ant rapresent the means of communication between Remote(s)
/// SendTo_ is used to add context to Message, it say what we need to do with that Message.
pub enum SendTo_<Message, Remote> {
    /// Used by proxies when Message must be relayed dowstream or upstream and we want to specify
    /// to which particula downstream or upstream we want to relay the message.
    ///
    /// When the message that we need to realy is the same message that we received should be used
    /// RelaySameMessageToRemote in order to save an allocation.
    RelayNewMessageToRemote(Arc<Mutex<Remote>>, Message),
    /// Used by proxies when Message must be relayed dowstream or upstream and we want to specify
    /// to which particula downstream or upstream we want to relay the message.
    ///
    /// Is used when we need to relay the same message the we received in order to save an
    /// allocation.
    RelaySameMessageToRemote(Arc<Mutex<Remote>>),
    /// Used by proxies when Message must be relayed dowstream or upstream and we do not want tospecify
    /// specify to which particula downstream or upstream we want to relay the message.
    ///
    /// This is used in proxies that do and Sv1 to Sv2 translation. The upstream is connected via
    /// an extdended channel that means that
    RelayNewMessage(Message),
    /// Used proxies clients and servers to directly respond to a received message.
    Respond(Message),
    Multiple(Vec<SendTo_<Message, Remote>>),
    /// Used by proxies, clients, and servers, when Message do not have to be used in any of the above way.
    /// If Message is still needed to be used in a non convetional way we use SendTo::None(Some(message))
    /// If we just want to discard it we can use SendTo::None(None)
    ///
    /// SendTo::None(Some(m)) could be used for example when we do not need to send the message,
    /// but we still need it for succesive handling/transformation.
    /// One of these cases are proxies that are connected to upstream via an extended channel (like the
    /// Sv1 <-> Sv2 translator). This because extended channel messages are always general for all
    /// the dowstream, where standard channel message can be specific for a particular dowstream.
    /// Another case is when 2 roles are implemented in the same software, like a pool that is
    /// both TP client and a Mining server, messages received by the TP client must be sent to the
    /// Mining Server than transformed in Mining messages and sent to the downstream.
    ///
    None(Option<Message>),
}

impl<Message, Remote> SendTo_<Message, Remote> {
    pub fn into_message(self) -> Option<Message> {
        match self {
            Self::RelayNewMessageToRemote(_, m) => Some(m),
            Self::RelaySameMessageToRemote(_) => None,
            Self::RelayNewMessage(m) => Some(m),
            Self::Respond(m) => Some(m),
            Self::Multiple(_) => None,
            Self::None(m) => m,
        }
    }
    pub fn into_remote(self) -> Option<Arc<Mutex<Remote>>> {
        match self {
            Self::RelayNewMessageToRemote(r, _) => Some(r),
            Self::RelaySameMessageToRemote(r) => Some(r),
            Self::RelayNewMessage(_) => None,
            Self::Respond(_) => None,
            Self::Multiple(_) => None,
            Self::None(_) => None,
        }
    }
}
