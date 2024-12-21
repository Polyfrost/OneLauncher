import type { Settings } from '@onelauncher/client/bindings';
import { getProgramInfo } from '@onelauncher/client';
import { useBeforeLeave } from '@solidjs/router';
import { bridge } from '~imports';
import { DEFAULT_THEME, setAppTheme, splitMergedTheme } from '~utils/theming';
import { type Context, createContext, createEffect, type ParentProps, Show, useContext } from 'solid-js';
import useCommand from './useCommand';

/**
 * Sync the launcher state (CSS, text etc) with the current settings
 */
export function syncSettings(settings: Settings) {
	document.body.classList.toggle('reduce-motion', settings.disable_animations);

	const split = splitMergedTheme(settings.theme || DEFAULT_THEME);
	setAppTheme(split.theme, split.variant);
}

interface SettingsControllerType {
	settings: () => Settings;
	saveOnLeave: (settings: () => Partial<Settings>) => void;
	save: (settings: Settings) => Promise<void>;
	refetch: () => void;
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
		save: async (settings) => {
			await bridge.commands.setSettings(settings);
			syncSettings(settings);
			await refetch();
		},
		refetch,
	};

	if (getProgramInfo().dev_build)
		// @ts-expect-error - Expose settings globally for debugging purposes
		window.onelauncherSettings = controller;

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
