use crate::api::proxy::event::LauncherEvent;

#[taurpc::procedures(event_trigger = LauncherEventEmitter)]
pub trait TauriLauncherEventApi {
	#[taurpc(event)]
	async fn send_event(event: LauncherEvent);
}

#[taurpc::ipc_type]
pub struct TauriLauncherEventApiImpl;

#[taurpc::resolvers]
impl TauriLauncherEventApi for TauriLauncherEventApiImpl {}
