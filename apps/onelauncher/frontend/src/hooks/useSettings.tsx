/* eslint-disable react-hooks/rules-of-hooks -- stfu */
/* eslint-disable ts/naming-convention -- please shut the fuck up i need to see if its working or not */
import type { Settings } from '@/bindings.gen';
import { bindings } from '@/main';
import { useCommand } from '@onelauncher/common';
import React, { createContext, useCallback, useContext, useEffect, useState } from 'react';

interface SettingsControllerType {
	settings: Settings | undefined;
	saveChangedSettings: () => void;
	settingsToSave: Partial<Settings> | null;
	setSettingsToSave: (settings: Partial<Settings> | null) => void;
	settingsChanged: boolean;
	setSettingsChanged: (changed: boolean) => void;
	createSetting: <K extends keyof Settings, V = Settings[K]>(name: K, value: V) => [V, (value: V) => void];
}

const SettingsContext = createContext<SettingsControllerType | null>(null);

interface SettingsProviderProps {
	children: React.ReactNode;
}

export function SettingsProvider({ children }: SettingsProviderProps) {
	const { data: settings } = useCommand('readSettings', bindings.core.readSettings);
	const [settingsChanged, setSettingsChanged] = useState(false);
	const [settingsToSave, setSettingsToSave] = useState<Partial<Settings> | null>(null);

	const filteredSettingsToSave = settingsToSave
		? Object.fromEntries(
			Object.entries(settingsToSave),
		) as Partial<Settings>
		: {};

	const patchedSettings: Settings = {
		...settings,
		...filteredSettingsToSave,
	} as Settings;

	const writeQuery = useCommand('writeSettings', () => bindings.core.writeSettings(patchedSettings), {
		enabled: false,
		subscribed: false,
	});

	useEffect(() => {
		const handleBeforeUnload = () => {
			setSettingsChanged(false);
		};

		window.addEventListener('beforeunload', handleBeforeUnload);
		return () => window.removeEventListener('beforeunload', handleBeforeUnload);
	}, []);

	const saveChangedSettings = () => {
		if (!settings)
			return;

		writeQuery.refetch();

		setSettingsChanged(false);
	};

	const handleSetSettingsToSave = useCallback((newSettings: Partial<Settings> | null) => {
		setSettingsToSave(newSettings);
		setSettingsChanged(true);
	}, []);

	const createSetting = useCallback(<K extends keyof Settings, V = Settings[K]>(
		name: K,
		initialValue: V,
	): [V, (value: V) => void] => {
		const [value, setValue] = useState<V>(initialValue);

		useEffect(() => {
			const didChange = settings?.[name] !== value;

			if (didChange && settings) {
				setSettingsToSave(prev => ({
					...prev,
					[name]: value,
				}));
				setSettingsChanged(true);
			}
		}, [value, name]);

		return [value, setValue];
	}, [settings]);

	const controller: SettingsControllerType = {
		settings,
		saveChangedSettings,
		settingsToSave,
		setSettingsToSave: handleSetSettingsToSave,
		settingsChanged,
		setSettingsChanged,
		createSetting,
	};

	if (settings === undefined)
		return null;

	return (
		<SettingsContext.Provider value={controller}>
			{children}

			{settingsChanged && (
				<div className="pointer-events-none absolute bottom-5 left-1/2 w-full flex flex-col items-center justify-center -translate-x-1/2">
					<div className="pointer-events-auto flex flex-row gap-1 border border-component-border rounded-md bg-component-bg p-3 px-4">
						You have unsaved changes!
						<button
							className="text-brand filter-brightness-130 hover:underline"
							onClick={saveChangedSettings}
							type="button"
						>
							Save Now
						</button>
					</div>
				</div>
			)}
		</SettingsContext.Provider>
	);
}

export function useSettings(): SettingsControllerType {
	const context = useContext(SettingsContext);

	if (!context)
		throw new Error('useSettings should be called inside its SettingsProvider');

	return context;
}

export default useSettings;
