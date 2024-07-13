import { type Context, type ParentProps, type Resource, Show, createContext, createEffect, useContext } from 'solid-js';
import useCommand from './useCommand';
import type { Settings } from '~bindings';
import { bridge } from '~imports';

const SettingsContext = createContext<Settings>() as Context<Settings>;

/**
 * Sync the launcher state (CSS, text etc) with the current settings
 */
export function syncSettings(settings: Settings) {
	document.body.classList.toggle('reduce-motion', settings.disable_animations);
}

export function getSettings(): Resource<Settings> | undefined {
	const [resource] = useCommand(bridge.commands.getSettings);
	return resource;
}

export function SettingsProvider(props: ParentProps) {
	const settings = getSettings();

	createEffect(() => {
		if (settings !== undefined && settings() !== undefined)
			syncSettings(settings!()!);
	});

	return (
		<Show when={settings !== undefined && settings() !== undefined}>
			<SettingsContext.Provider value={settings!()!}>
				{props.children}
			</SettingsContext.Provider>
		</Show>
	);
}

export function useSettingsContext() {
	const context = useContext(SettingsContext);

	if (!context)
		throw new Error('useSettingsContext should be called inside its SettingsProvider');

	const contextProxy = new Proxy(context, {
		set(target, key, value) {
			// @ts-expect-error typescript
			target[key] = value;
			save(target); // TODO: Possibly make a "Save confirmation" instead of writing to the settings file every time?
			return true;
		},
	});

	return contextProxy;
}

function save(settings: Settings) {
	bridge.commands.setSettings(settings);
	syncSettings(settings);
}

export default useSettingsContext;
