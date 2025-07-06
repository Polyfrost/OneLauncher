import type { ReactNode } from 'react';
import { bindings } from '@/main';
import { randomString } from '@/utils/index';
import { createContext, useContext, useState } from 'react';

export interface NotificationData {
	title: string;
	message: string;
	fraction?: number | undefined;
}

type Notifications = Record<string, NotificationData>;

export interface HookReturn {
	list: Notifications;
	set: (id: string, data: NotificationData) => void;
	create: (data: NotificationData) => void;
	remove: (id: string) => void;
	clear: () => void;
}

interface NotificationContextValue {
	notifications: Notifications;
	setNotifications: React.Dispatch<React.SetStateAction<Notifications>>;
}

const NotificationContext = createContext<NotificationContextValue | undefined>(undefined);

interface NotificationProviderProps {
	children: ReactNode;
}

export function NotificationProvider({ children }: NotificationProviderProps) {
	const [notifications, setNotifications] = useState<Notifications>({});

	bindings.events.ingress.on((e) => {
		setNotifications(prev => ({
			...prev,
			[e.id]: {
				title: `${e.ingress_type}`,
				message: e.message,
			},
		}));
	}).then(u => u());

	const ctx = {
		notifications,
		setNotifications,
	};

	return (
		<NotificationContext.Provider value={ctx}>
			{children}
		</NotificationContext.Provider>
	);
}

function useNotifications(): HookReturn {
	const context = useContext(NotificationContext);

	if (!context)
		throw new Error('useNotifications must be used within a NotificationProvider');

	const { notifications, setNotifications } = context;

	const ctx: HookReturn = {
		list: notifications,
		set: (id, data) => setNotifications(prev => ({ ...prev, [id]: data })),
		create: data => ctx.set(randomString(6), data),
		remove: id => setNotifications((prev) => {
			const { [id]: removed, ...rest } = prev;
			return rest;
		}),
		clear: () => setNotifications({}),
	};

	return ctx;
}

export default useNotifications;
