import type { UnlistenFn } from '@tauri-apps/api/event';
import type { NotificationData } from '~ui/components/overlay/notifications/NotificationComponent';
import { events } from '@onelauncher/client/bindings';
import { type Accessor, type Context, createContext, createSignal, onCleanup, onMount, type ParentProps, type Signal, useContext } from 'solid-js';

type Notifications = Record<string, NotificationData>;

interface HookReturn {
	list: Accessor<Notifications>;
	set: (id: string, data: NotificationData) => void;
	clear: () => void;
}

const NotificationContext = createContext() as Context<Signal<Notifications>>;

export function NotificationProvider(props: ParentProps) {
	const [notifications, setNotifications] = createSignal<Notifications>({});
	let unlisten: UnlistenFn | undefined;

	onMount(() => {
		document.addEventListener('keypress', (e) => {
			if (e.key === 'n')
				setNotifications(notifications => ({
					...notifications,
					[Math.random().toString()]: {
						title: 'Test',
						message: 'This is a test notification',
					},
				}));
		});

		events.ingressPayload.listen((e) => {
			setNotifications(notifications => ({
				...notifications,
				[e.payload.ingress_uuid]: {
					title: e.payload.event.type.replaceAll('_', ' '),
					message: e.payload.message,
					fraction: e.payload.fraction ?? undefined,
				},
			}));
		}).then(u => unlisten = u);
	});

	onCleanup(() => {
		unlisten?.();
	});

	return (
		<NotificationContext.Provider value={[notifications, setNotifications]}>
			{props.children}
		</NotificationContext.Provider>
	);
}

function useNotifications(): HookReturn {
	const [notifications, setNotifications] = useContext(NotificationContext);

	return {
		list: notifications,
		set: (id, data) => setNotifications(notifications => ({ ...notifications, [id]: data })),
		clear: () => setNotifications({}),
	};
}

export default useNotifications;
