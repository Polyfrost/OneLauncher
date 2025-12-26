import type { UnlistenFn } from '@tauri-apps/api/event';
import { listen } from '@tauri-apps/api/event';
import { bindings } from '../main';

export interface Update {
	version: string;
}

export type UpdateEvent =
	| { status: 'loading' }
	| { status: 'error'; error: string }
	| { status: 'updateAvailable'; update: Update }
	| { status: 'noUpdateAvailable' }
	| { status: 'installing' };

export async function checkForUpdate(): Promise<Update | null> {
	return await bindings.oneclient.checkForUpdate();
}

export async function installUpdate(): Promise<void> {
	await bindings.oneclient.installUpdate();
}

export async function listenForUpdateEvents(
	callback: (event: UpdateEvent) => void,
): Promise<UnlistenFn> {
	return await listen<UpdateEvent>('updater', (event) => {
		callback(event.payload);
	});
}
