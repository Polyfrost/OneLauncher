import { useBeforeLeave } from '@solidjs/router';
import { bridge } from '~imports';
import { type Context, createContext, createEffect, type ParentProps, Show, useContext } from 'solid-js';
import type { Settings } from '@onelauncher/client/bindings';
import useCommand from './useCommand';

/**
 * Sync the launcher state (CSS, text etc) with the current settings
 */
export function syncSettings(settings: Settings) {
	document.body.classList.toggle('reduce-motion', settings.disable_animations);
}

interface SettingsControllerType {
	settings: () => Settings;
	saveOnLeave: (settings: () => Partial<Settings>) => void;
	save: (settings: Settings) => void;
}

const SettingsContext = createContext() as Context<SettingsControllerType>;

export function SettingsProvider(props: ParentProps) {
	const [settings, { refetch }] = useCommand(() => bridge.commands.getSettings());

	createEffect(() => {
		if (settings !== undefined && settings() !== undefined)
			syncSettings(settings!()!);
	});

	const controller: SettingsControllerType = {
		settings: () => settings!()!,
		saveOnLeave: (settings) => {
			useBeforeLeave(() => {
				controller.save({
					...controller.settings(),
					...settings(),
				});
			});
		},
		save: (settings) => {
			bridge.commands.setSettings(settings).then(() => {
				syncSettings(settings);
				refetch();
			});
		},
	};

	return (
		<Show when={settings !== undefined && settings() !== undefined}>
			<SettingsContext.Provider value={controller}>
				{props.children}
			</SettingsContext.Provider>
		</Show>
	);
}

export function useSettings() {
	const context = useContext(SettingsContext);

	if (!context)
		throw new Error('useSettingsContext should be called inside its SettingsProvider');

	return context;
}

export default useSettings;
