import { useSettings } from '@/hooks/useSettings';
import { bindings } from '@/main';
import { Button } from '@onelauncher/common/components';
import { Link } from '@tanstack/react-router';
import { dataDir, join } from '@tauri-apps/api/path';
import { BatteryEmptyIcon, Code02Icon, FolderIcon, LinkExternal01Icon, Truck01Icon } from '@untitled-theme/icons-react';
import { useEffect, useState } from 'react';
import SettingsRow from '../SettingsRow';
import SettingsSwitch from '../SettingSwitch';
import { Overlay } from './Overlay';

interface DevInfo {
	inDev: boolean;
	platform: string;
	arch: string;
	family: string;
	locale: string;
	type: string;
	version: string;
}

function useDevInfo(): DevInfo {
	const [devInfo, setDevInfo] = useState<DevInfo>({
		inDev: false,
		platform: 'UNKNOWN',
		arch: 'UNKNOWN',
		family: 'UNKNOWN',
		locale: 'UNKNOWN',
		type: 'UNKNOWN',
		version: 'UNKNOWN',
	});

	useEffect(() => {
		const fetchDevInfo = async () => {
			const [
				inDev,
				platform,
				arch,
				family,
				locale,
				type,
				version,
			] = await Promise.all([
				bindings.debug.isInDev(),
				bindings.debug.getPlatform(),
				bindings.debug.getArch(),
				bindings.debug.getFamily(),
				bindings.debug.getLocale(),
				bindings.debug.getType(),
				bindings.debug.getVersion(),
			]);

			setDevInfo({
				inDev,
				platform,
				arch,
				family,
				locale: locale ?? 'UNKNOWN',
				type,
				version,
			});
		};

		void fetchDevInfo();
	}, []);

	return devInfo;
}

export function SuperSecretDevOptions() {
	const { createSetting } = useSettings();
	const devInfo = useDevInfo();
	const [launcherDir, setLauncherDir] = useState('');

	useEffect(() => {
		(async () => {
			setLauncherDir(await join(await dataDir(), 'OneClient'));
		})();
	}, []);

	const openLauncherDir = () => bindings.core.open(launcherDir);

	return (
		<Overlay.Dialog>
			<Overlay.Title>Super Secret Dev Options</Overlay.Title>

			<div>
				<SettingsRow description="Enable The Tanstack Dev Tools" icon={<Code02Icon />} title="Tanstack Dev Tools">
					<SettingsSwitch setting={createSetting('show_tanstack_dev_tools')} />
				</SettingsRow>

				<SettingsRow description="Open Dev Tools" icon={<Code02Icon />} title="Open Dev Tools">
					<Button onPress={bindings.debug.openDevTools} size="normal">Open</Button>
				</SettingsRow>

				<SettingsRow description="Use Parallel Mod Downloading for speed. This can create some issues sometimes" icon={<BatteryEmptyIcon />} title="Use Parallel Mod Downloading">
					<SettingsSwitch setting={createSetting('parallel_mod_downloading')} />
				</SettingsRow>

				<SettingsRow description="Skip Onboarding" icon={<Truck01Icon />} title="Skip Onboarding">
					<Link to="/onboarding/finished">
						<Button size="normal">Skip</Button>
					</Link>
				</SettingsRow>
				<SettingsRow description={launcherDir} icon={<FolderIcon />} title="Launcher Folder">
					<Button onClick={openLauncherDir} size="normal">
						<LinkExternal01Icon />
						{' '}
						Open
					</Button>
				</SettingsRow>
			</div>

			<div>
				<p>inDev: {devInfo.inDev ? 'yes' : 'no'}</p>
				<p>Platform: {devInfo.platform}</p>
				<p>Arch: {devInfo.arch}</p>
				<p>Family: {devInfo.family}</p>
				<p>Locale: {devInfo.locale}</p>
				<p>Type: {devInfo.type}</p>
				<p>Version: {devInfo.version}</p>
			</div>
		</Overlay.Dialog>
	);
}
