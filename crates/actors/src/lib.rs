use dkg_engine_actor::DKGEngineError;
use election_actor::ElectionEngineError;
use messages::actor_type::ActorType;
use ractor::{ActorProcessingErr, ActorRef};

pub mod app_state_actor;
pub mod dkg_engine_actor;
pub mod election_actor;
pub mod gossip_engine_actor;

pub const THRESHOLD: u8 = 2;

#[derive(Debug)]
pub enum ProcessMessageError {
    ElectionEngineError(ElectionEngineError),
    DKGEngineError(DKGEngineError),
    Custom(String),
}

impl From<ElectionEngineError> for ProcessMessageError {
    fn from(err: ElectionEngineError) -> Self {
        ProcessMessageError::ElectionEngineError(err)
    }
}

impl From<DKGEngineError> for ProcessMessageError {
    fn from(err: DKGEngineError) -> Self {
        ProcessMessageError::DKGEngineError(err)
    }
}

pub fn get_actor_ref<A>(
    actor_type: ActorType,
    error_msg: &'static str,
) -> Result<ActorRef<A>, ActorProcessingErr> {
    let registry = ractor::registry::where_is(actor_type.to_string());
    match registry {
        Some(actor_ref) => Ok(actor_ref.into()),
        None => Err(ActorProcessingErr::from(error_msg)),
    }
}

#[macro_export]
macro_rules! cast_message {
    ($actor_type:expr, $message:expr) => {{
        let actor_ref_result = $crate::get_actor_ref($actor_type, "Unable to acquire actor");
        if let Ok(actor_ref) = actor_ref_result {
            actor_ref.cast($message).unwrap_or_else(|err| {
                println!("Failed to cast message: {}", err);
            });
        } else if let Err(err) = actor_ref_result {
            println!("Failed to get actor reference: {}", err);
        }
    }};
}
