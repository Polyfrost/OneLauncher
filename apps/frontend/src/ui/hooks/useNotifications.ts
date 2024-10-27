import type { UnlistenFn } from '@tauri-apps/api/event';
import type { NotificationData } from '~ui/components/overlay/notifications/NotificationComponent';
import { events } from '@onelauncher/client/bindings';
import { createSignal, onCleanup, onMount, type Signal } from 'solid-js';

type Notifications = Record<string, NotificationData>;
type OnUpdateFn = () => unknown;

function useNotifications(onUpdate?: OnUpdateFn): Signal<Notifications> {
	const [notifications, setNotifications] = createSignal<Notifications>({});
	let unlisten: UnlistenFn | undefined;

	async function update(id: string, data: NotificationData) {
		setNotifications((notifications) => {
			notifications[id] = data;
			return {
				...notifications,
			};
		});

		if (onUpdate)
			onUpdate();
	}

	onMount(() => {
		events.ingressPayload.listen(e => update(e.payload.ingress_uuid, {
			title: e.payload.event.type.replaceAll('_', ' '),
			message: e.payload.message,
			fraction: e.payload.fraction ?? undefined,
		})).then(u => unlisten = u);
	});

	onCleanup(() => {
		unlisten?.();
	});

	return [notifications, setNotifications];
}

export default useNotifications;
