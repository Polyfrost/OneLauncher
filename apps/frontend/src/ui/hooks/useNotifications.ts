import { type Signal, createSignal, onMount } from 'solid-js';

type Notifications = Core.Notification[];
type OnUpdateFn = (event: Core.NotificationEvents) => unknown;

function useNotifications(onUpdate?: OnUpdateFn): Signal<Notifications> {
	const [notifications, setNotifications] = createSignal<Notifications>([]);

	async function update(event: Core.NotificationEvents) {
		const notis: Notifications = []; // await manager.getNotifications();
		setNotifications(notis);

		if (onUpdate)
			onUpdate(event);
	}

	onMount(() => {
		update('init');

		// manager.on('added', () => update('added'));
		// manager.on('removed', () => update('removed'));
		// manager.on('cleared', () => update('cleared'));
		// manager.on('modified', () => update('modified'));
	});

	return [notifications, setNotifications];
}

export default useNotifications;
