import { invoke } from '@tauri-apps/api/core';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { listen } from '@tauri-apps/api/event';

export enum NotificationType {
	Info = 'Info',
	Alert = 'Alert',
	Success = 'Success',
	Download = 'Download',
	DownloadSuccess = 'DownloadSuccess',
	DownloadError = 'DownloadError',
	Refresh = 'Refresh',
}

let _notifications: Core.Notification[] = [];

class _Emitter {
	private listeners: ((event: string) => unknown)[] = [];

	public emit(event: Core.NotificationEvents) {
		this.listeners.forEach(value => value(event));
	}

	public on(callback: (event: string) => unknown) {
		this.listeners.push(callback);
	}
}

const emitter = new _Emitter();

export async function addNotification(notification: MakeOptional<Core.Notification, 'id' | 'created_at'>): Promise<void> {
	_notifications.push({
		id: _notifications.length + 1,
		created_at: Math.floor(Date.now() / 1000),
		...notification,
	});
	emitter.emit('added');
	// await invoke('plugin:onelauncher|notifications_add', {
	// 	notification: {
	// 		created_at: Math.floor(Date.now() / 1000),
	// 		...notification,
	// 	},
	// });
}

export async function removeNotificationById(id: number): Promise<void> {
	const index = _notifications.findIndex(noti => noti.id === id);
	if (index > -1) {
		_notifications.splice(index, 1);
		emitter.emit('removed');
	}
}

export async function removeNotificationByIndex(index: number): Promise<void> {
	_notifications.splice(index, 1);
	emitter.emit('removed');
	// await invoke('plugin:onelauncher|notifications_remove_by_index', { index });
}

export async function clearNotifications(): Promise<void> {
	_notifications = [];
	emitter.emit('cleared');
	// await invoke('plugin:onelauncher|notifications_clear');
}

export async function getNotifications(): Promise<Core.Notification[]> {
	return [..._notifications];
	// return await invoke('plugin:onelauncher|notifications_get');
}

export async function on<
    T extends Core.NotificationEvents,
>(
	event: T,
	callback: () => void,
): Promise<UnlistenFn> {
	emitter.on((e) => {
		if (event === e)
			callback();
	});
	return () => {};
	// return await listen<string>(`notifications_event`, (e) => {
	// 	if (e.payload.toLowerCase() === event)
	// 		callback();
	// });
};
