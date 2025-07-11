import type { ReactNode } from 'react';
import { bindings } from '@/main';
import { randomString } from '@/utils/index';
import { createContext, memo, useContext, useEffect, useMemo, useState } from 'react';

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

const defaultContextValue: NotificationContextValue = {
	notifications: {},
	setNotifications: () => { },
};

const NotificationContext = createContext<NotificationContextValue>(defaultContextValue);

interface NotificationProviderProps {
	children: ReactNode;
}

export const NotificationProvider = memo(({ children }: NotificationProviderProps) => {
	const [notifications, setNotifications] = useState<Notifications>({});

	useEffect(() => {
		let cleanup: (() => void) | undefined;

		bindings.events.ingress.on((e) => {
			if (typeof e.ingress_type === 'object') {
				if ('PrepareCluster' in e.ingress_type) {
					const { cluster_name } = e.ingress_type.PrepareCluster;
					setNotifications(prev => ({
						...prev,
						[e.id]: {
							title: `Preparing ${cluster_name}`,
							message: e.message,
							fraction: e.percent ?? undefined,
						},
					}));
				}
			}
			else {
				setNotifications(prev => ({
					...prev,
					[e.id]: {
						title: `${e.ingress_type}`,
						message: e.message,
						fraction: e.percent ?? undefined,
					},
				}));
			}
		}).then((u) => {
			cleanup = u;
		}).catch(console.error);

		return () => {
			cleanup?.();
		};
	}, []);

	const contextValue = useMemo(() => ({
		notifications,
		setNotifications,
	}), [notifications, setNotifications]);

	return (
		<NotificationContext.Provider value={contextValue}>
			{children}
		</NotificationContext.Provider>
	);
});

function useNotifications(): HookReturn {
	const context = useContext(NotificationContext);
	const { notifications, setNotifications } = context;

	const isDefaultContext = setNotifications === defaultContextValue.setNotifications;

	if (isDefaultContext) {
		if (import.meta.env.DEV) {
			console.warn('useNotifications: Using default context (likely due to HMR or missing provider)');
			return {
				list: {},
				set: () => { },
				create: () => { },
				remove: () => { },
				clear: () => { },
			};
		}
		throw new Error('useNotifications must be used within a NotificationProvider');
	}

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
