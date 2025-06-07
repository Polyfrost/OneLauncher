#[taurpc::procedures(path = "events", event_trigger = LauncherEventEmitter)]
pub trait TauriLauncherEventApi {
	#[taurpc(event)]
	#[taurpc(alias = "ingress")]
	async fn ingress(event: crate::api::ingress::IngressPayload);

	#[taurpc(event)]
	#[taurpc(alias = "message")]
	async fn message(event: crate::api::proxy::message::MessagePayload);

	#[taurpc(event)]
	#[taurpc(alias = "process")]
	async fn process(event: crate::api::processes::ProcessPayload);
}

#[taurpc::ipc_type]
pub struct TauriLauncherEventApiImpl;

#[taurpc::resolvers]
impl TauriLauncherEventApi for TauriLauncherEventApiImpl {}
