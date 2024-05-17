import { createEffect, createRoot, onMount } from 'solid-js';
import { createStore } from 'solid-js/store';

export interface AppSettingsStore {
	reducedMotion: boolean;
	closeDialog: boolean;
};

function globalStore() {
	// TODO: Much better management of this, currently temporary
	const [settings, setSettings] = createStore<AppSettingsStore>({
		reducedMotion: false,
		closeDialog: true,
	});

	function set<K extends keyof AppSettingsStore>(key: K, value: AppSettingsStore[K]) {
		window.localStorage.setItem(key, value.toString());
	}

	onMount(() => {
		setSettings('reducedMotion', window.localStorage.getItem('reducedMotion') === 'true');
		setSettings('closeDialog', window.localStorage.getItem('closeDialog') === 'true');
	});

	createEffect(() => {
		// Reduced Motion
		set('reducedMotion', settings.reducedMotion);
		document.body.classList.toggle('reduce-motion', settings.reducedMotion);

		// Close Dialog
		set('closeDialog', settings.closeDialog);
	});

	return { settings, setSettings };
}

export default createRoot(globalStore);
