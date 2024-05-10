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

export async function addNotification(notification: MakeOptional<Core.Notification, 'created_at'>): Promise<void> {
	await invoke('plugin:onelauncher|notifications_add', {
		notification: {
			created_at: Math.floor(Date.now() / 1000),
			...notification,
		},
	});
}

export async function removeNotification(index: number): Promise<void> {
	await invoke('plugin:onelauncher|notifications_remove_by_index', { index });
}

export async function clearNotifications(): Promise<void> {
	await invoke('plugin:onelauncher|notifications_clear');
}

export async function getNotifications(): Promise<Core.Notification[]> {
	return await invoke('plugin:onelauncher|notifications_get');
}

export async function on<
    T extends Core.NotificationEvents,
>(
	event: T,
	callback: () => void,
): Promise<UnlistenFn> {
	return await listen<string>(`notifications_event`, (e) => {
		if (e.payload.toLowerCase() === event)
			callback();
	});
};
