import { type Signal, createSignal, onMount } from 'solid-js';
import * as manager from '~bridge/notifications';

type Notifications = Core.Notification[];
type OnUpdateFn = (event: Core.NotificationEvents) => unknown;

function useNotifications(onUpdate?: OnUpdateFn): Signal<Notifications> {
	const [notifications, setNotifications] = createSignal<Notifications>([]);

	async function update(event: Core.NotificationEvents) {
		const notis = await manager.getNotifications();
		setNotifications(notis);

		if (onUpdate)
			onUpdate(event);
	}

	onMount(() => {
		update('init');

		manager.on('added', () => update('added'));
		manager.on('removed', () => update('removed'));
		manager.on('cleared', () => update('cleared'));
	});

	return [notifications, setNotifications];
}

export default useNotifications;
