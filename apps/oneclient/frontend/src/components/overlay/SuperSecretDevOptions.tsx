import { Overlay, RawDebugInfo, SettingsRow, SettingsSwitch, useDebugInfo } from '@/components';
import { useSettings } from '@/hooks/useSettings';
import { bindings } from '@/main';
import { Button } from '@onelauncher/common/components';
import { Link } from '@tanstack/react-router';
import { dataDir, join } from '@tauri-apps/api/path';
import { BatteryEmptyIcon, Code02Icon, FolderIcon, LinkExternal01Icon, Truck01Icon } from '@untitled-theme/icons-react';
import { useEffect, useState } from 'react';

export function SuperSecretDevOptions() {
	const { createSetting } = useSettings();
	const debugInfo = useDebugInfo();

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

			<RawDebugInfo debugInfo={debugInfo} />
		</Overlay.Dialog>
	);
}
