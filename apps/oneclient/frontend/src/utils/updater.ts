import type { UnlistenFn } from '@tauri-apps/api/event';
import { bindings } from '@/main';
import { listen } from '@tauri-apps/api/event';
import { useEffect } from 'react';

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

export function useAutoUpdater() {
	useEffect(() => {
		void (async () => {
			try {
				const update = await checkForUpdate();
				if (!update)
					return;

				// eslint-disable-next-line no-console -- Used for debugging - aka important
				console.log('Update found on initial check:', update.version);

				try {
					await installUpdate();
				}
				catch (e) {
					console.error('Failed to install update:', e);
				}
			}
			catch (e) {
				console.error('Failed to check for update:', e);
			}
		})();
	}, []);
}
