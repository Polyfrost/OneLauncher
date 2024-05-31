import { invoke } from '@tauri-apps/api/core';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { listen } from '@tauri-apps/api/event';

// TODO: Refactor notifications
// - getNotifications should return a list of notifications
// - event "update" + "add" + "remove" should emit the event AS WELL AS emitting
//      the ID of the notification / the entire notification data
//      so that the frontend can easily find the notification reference and update it without losing reactivity

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

export async function updateNotification(id: number, notification: Partial<Core.Notification>): Promise<void> {
	const index = _notifications.findIndex(noti => noti.id === id);
	if (index < 0) {
		console.log('no index');
		return;
	}

	const found = _notifications[index];
	if (!found) {
		console.log('not found');
		return;
	}

	const newNoti = {
		...found,
		...notification,
	};

	_notifications[index] = newNoti;
	emitter.emit('modified');
}

export async function addNotification(notification: MakeOptional<Core.Notification, 'id' | 'created_at'>): Promise<number> {
	const id = notification.id || _notifications.length + 1;
	_notifications.push({
		id,
		created_at: Math.floor(Date.now() / 1000),
		...notification,
	});
	emitter.emit('added');
	return id;

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
	return [..._notifications]; // Destructured to mock the way backend <-> frontend works. (new list gets sent)
	// return await invoke('plugin:onelauncher|notifications_get');
}

export async function getNotificationById(id: number): Promise<Core.Notification | undefined> {
	return _notifications.find(noti => noti.id === id);
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
