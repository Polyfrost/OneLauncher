import type { Settings } from '@/bindings.gen';
import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';
import { useMemo } from 'react';

export function useSettings() {
	const settings = useCommandSuspense(['readSettings'], bindings.core.readSettings);
	const setting = useMemo(() => <TKey extends keyof Settings>(key: TKey) => settings.data[key], [settings]);

	const setSetting = useMemo(() => <TKey extends keyof Settings>(key: TKey, value: Settings[TKey]) => {
		bindings.core.writeSettings({
			...settings.data,
			[key]: value,
		});
		settings.refetch();
	}, [settings]);

	type Setter<TKey extends keyof Settings> = (value: Settings[TKey]) => void;

	const createSetting = useMemo(() => <TKey extends keyof Settings>(key: TKey): [Settings[TKey], Setter<TKey>] => {
		return [setting(key), value => setSetting(key, value)];
	}, [setting]);

	return { setting, setSetting, createSetting };
}
