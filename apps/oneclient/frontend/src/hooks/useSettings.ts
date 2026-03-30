import type { ClusterModel, ProfileUpdate, Settings } from '@/bindings.gen';
import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';
import { useEffect, useMemo, useState } from 'react';

export function useSettings() {
	const settings = useCommandSuspense(['readSettings'], bindings.core.readSettings);
	const setting = useMemo(() => {
		return function setting<TKey extends keyof Settings>(key: TKey) {
			return settings.data[key];
		};
	}, [settings]);

	const setSetting = useMemo(() => {
		return async function setSetting<TKey extends keyof Settings>(key: TKey, value: Settings[TKey]) {
			await bindings.core.writeSettings({
				...settings.data,
				[key]: value,
			});
			await settings.refetch();
		};
	}, [settings]);

	type Setter<TKey extends keyof Settings> = (value: Settings[TKey]) => void;

	const createSetting = useMemo(() => {
		return function createSetting<TKey extends keyof Settings>(key: TKey): [Settings[TKey], Setter<TKey>] {
			return [setting(key), value => setSetting(key, value)];
		};
	}, [setting, setSetting]);

	return { setting, setSetting, createSetting };
}

const emptyUpdate: ProfileUpdate = {
	res: null,
	force_fullscreen: null,
	mem_max: null,
	launch_args: null,
	launch_env: null,
	hook_pre: null,
	hook_wrapper: null,
	hook_post: null,
};

export function useClusterProfile(cluster: ClusterModel) {
	const [profileName, setProfileName] = useState(cluster.setting_profile_name);
	const profileSrc = useCommandSuspense(['getProfileOrDefault', profileName], () => bindings.core.getProfileOrDefault(profileName));
	const [profile, setProfile] = useState(profileSrc.data);

	useEffect(() => {
		setProfile(profileSrc.data);
	}, [profileSrc.data]);

	const updateProfile = useMemo(() =>
		async (update: Partial<ProfileUpdate>) => {
			setProfile(profile => ({ ...profile, ...update }));
			let name = profileName;
			if (!profileName || profileSrc.data.name !== profileName) {
				name = globalThis.crypto.randomUUID();
				await bindings.core.createSettingsProfile(name);
				await bindings.core.updateClusterById(cluster.id, {
					setting_profile_name: name,
					name: null,
					icon_url: null,
				});
				setProfileName(name);
			}
			if (!name)
				throw new Error('No settings profile name');
			await bindings.core.updateClusterProfile(name, { ...emptyUpdate, ...update });
			profileSrc.refetch();
		}, [profileName, cluster.id, profileSrc]);
	// const updateProfile = useMemo(() => (update: Partial<ProfileUpdate>) => setProfile({ ...profile.data, ...update }), [setProfile, profile.data]);
	return { profile, setProfile, updateProfile };
}
