import { type Context, type ParentProps, Show, createContext, createEffect, useContext } from 'solid-js';
import { useBeforeLeave } from '@solidjs/router';
import useCommand from './useCommand';
import type { Settings } from '~bindings';
import { bridge } from '~imports';

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
	const [settings, { refetch }] = useCommand(bridge.commands.getSettings);

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

export function useSettingsContext() {
	const context = useContext(SettingsContext);

	if (!context)
		throw new Error('useSettingsContext should be called inside its SettingsProvider');

	return context;
}

export default useSettingsContext;
