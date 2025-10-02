// import DiscordIcon from '@/assets/logos/discord.svg';
import SettingsRow from '@/components/SettingsRow';
import SettingsSwitch from '@/components/SettingSwitch';
import { useSettings } from '@/hooks/useSettings';
import { bindings } from '@/main';
// import useSettings from '@/hooks/useSettings';
import { Button } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { dataDir, join } from '@tauri-apps/api/path';
import { FolderIcon, Link03Icon, LinkExternal01Icon } from '@untitled-theme/icons-react';
import { useEffect, useState } from 'react';
import Sidebar from './route';

export const Route = createFileRoute('/app/settings/')({
	component: RouteComponent,
});

function RouteComponent() {
	const { createSetting } = useSettings();
	const [launcherDir, setLauncherDir] = useState('');

	useEffect(() => {
		(async () => {
			setLauncherDir(await join(await dataDir(), 'OneLauncher'));
		})();
	}, []);

	const openLauncherDir = () => bindings.core.open(launcherDir);

	return (
		<Sidebar.Page>
			<div className="h-full">
				<h1>General</h1>

				<SettingsRow
					description="Enable Discord Rich Presence."
					icon={<Link03Icon />}
					title="Discord RPC"
				>
					<SettingsSwitch setting={createSetting('discord_enabled')} />
				</SettingsRow>

				{/* <SettingsRow
					description="Hide the confirmation dialog when closing the launcher."
					icon={<XIcon />}
					title="Hide Close Dialog"
				>
					<Switch />
				</SettingsRow>

				<SettingsRow
					description="Sends errors and crash logs using Sentry to help developers fix issues. (// TODO)"
					icon={<AlertSquareIcon />}
					title="Error Analytics"
				>
					<Toggle
						checked={() => !(settings().disable_analytics ?? false)}
						onChecked={value => settings().disable_analytics = !value}
					/>
				</SettingsRow> */}

				<SettingsRow.Header>Folders and Files</SettingsRow.Header>
				<SettingsRow
					description={launcherDir}
					icon={<FolderIcon />}
					title="Launcher Folder"
				>
					<Button onClick={openLauncherDir} size="normal">
						<LinkExternal01Icon />
						{' '}
						Open
					</Button>
				</SettingsRow>
			</div>
		</Sidebar.Page>
	);
}
