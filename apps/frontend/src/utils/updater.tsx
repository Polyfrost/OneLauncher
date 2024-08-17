import { listen } from '@tauri-apps/api/event';
import { commands } from '~bindings';

declare global {
	interface Window {
		__LAUNCHER_UPDATER__?: true;
		__ONELAUNCHER_VERSION__: string;
	}
}

export interface Update { version: string }
export type UpdateStore =
	| { status: 'idle' }
	| { status: 'loading' }
	| { status: 'error' }
	| { status: 'updateAvailable'; update: Update }
	| { status: 'noUpdateAvailable' }
	| { status: 'installing' };

export function createUpdater() {
	if (!window.__LAUNCHER_UPDATER__)
		return;

	const updateStore: UpdateStore = { status: 'idle' };
	listen<UpdateStore>('updater', e => Object.assign(updateStore, e.payload));
	const onInstallCallbacks = new Set<() => void>();

	async function checkForUpdate() {
		const result = await commands.checkForUpdate();

		if (result.status === 'error') {
			console.error('[updater]: ', result.error);
			return null;
		}

		if (!result.data)
			return null;

		// TODO: ui
		// TODO: cb

		return result.data;
	}

	function installUpdate() {
		for (const cb of onInstallCallbacks) cb();
		// TODO: ui/toast
		return commands.installUpdate();
	}

	const ONELAUNCHER_VERSION_LOCALSTORAGE = 'onelauncher-version';
	async function updatedCheck() {
		const version = window.__ONELAUNCHER_VERSION__;
		const lastVersion = localStorage.getItem(ONELAUNCHER_VERSION_LOCALSTORAGE);
		const updaterStore = { tagline: null };

		if (!lastVersion)
			return;

		if (lastVersion !== version) {
			localStorage.setItem(ONELAUNCHER_VERSION_LOCALSTORAGE, version);

			try {
				const req = await fetch(`${import.meta.env.VITE_LANDING_ORIGIN}/api/releases/${version}`);
				const { frontmatter } = await req.json();
				updaterStore.tagline = frontmatter?.tagline;
			}
			catch (error) {
				console.warn('[updater]: failed to fetch release info');
				console.error('[updater]: ', error);
			}

			// TODO: ui/toast/popup
		}
	}

	return {
		checkForUpdate,
		installUpdate,
		updatedCheck,
	};
}
