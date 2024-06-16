import { type Context, type ParentProps, type Resource, Show, createContext, useContext } from 'solid-js';
import useCommand from './useCommand';
import type { Settings } from '~bindings';
import { bridge } from '~index';

const SettingsContext = createContext<Settings>() as Context<Settings>;

export function getSettings(): Resource<Settings> | undefined {
	const [resource] = useCommand(bridge.commands.getSettings);
	return resource;
}

export function SettingsProvider(props: ParentProps) {
	const settings = getSettings();

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

	return context;
}

export default useSettingsContext;
