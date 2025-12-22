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
	const logRunningProcesses = () => {
		(async () => {
			const running = await bindings.core.getRunningProcesses();
			// eslint-disable-next-line no-console -- Designed to log
			console.log(running);
		})();
	};

	return (
		<Overlay.Dialog>
			<Overlay.Title>Super Secret Dev Options</Overlay.Title>

			<div>
				<SettingsRow description="Enable The Tanstack Dev Tools" icon={<Code02Icon />} title="Tanstack Dev Tools">
					<SettingsSwitch setting={createSetting('show_tanstack_dev_tools')} />
				</SettingsRow>

				<SettingsRow description="Use Parallel Mod Downloading for speed. This can create some issues sometimes" icon={<BatteryEmptyIcon />} title="Use Parallel Mod Downloading">
					<SettingsSwitch setting={createSetting('parallel_mod_downloading')} />
				</SettingsRow>

				<div className="grid grid-cols-3 gap-3 justify-items-center">
					<Button onPress={bindings.debug.openDevTools} size="normal">Dev Tools</Button>

					<Link to="/onboarding/finished">
						<Button size="normal">Skip Onboarding</Button>
					</Link>

					<Button onClick={openLauncherDir} size="normal">Open Launcher Data</Button>

					<Button onClick={logRunningProcesses} size="normal">Console log processes</Button>
				</div>
			</div>

			<RawDebugInfo debugInfo={debugInfo} />
		</Overlay.Dialog>
	);
}
