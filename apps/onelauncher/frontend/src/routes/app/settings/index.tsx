import DiscordIcon from '@/assets/logos/discord.svg';
import SettingsRow from '@/components/SettingsRow';
import useSettings from '@/hooks/useSettings';
import { Button, Switch } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { dataDir } from '@tauri-apps/api/path';
import { openPath as open } from '@tauri-apps/plugin-opener';
import { FolderIcon, LinkExternal01Icon } from '@untitled-theme/icons-react';
import Sidebar from './route';

export const Route = createFileRoute('/app/settings/')({
	component: RouteComponent,
});

function RouteComponent() {
	const { settings, createSetting } = useSettings();

	const openLauncherDir = async () => {
		const dir = await dataDir();

		open(`${dir}/OneLauncher`);
	};

	const [discordRpc, setDiscordRpc] = createSetting('discord_enabled', settings?.discord_enabled);

	return (
		<Sidebar.Page>
			<div className="h-full">
				<h1>General Settings</h1>

				<SettingsRow
					description="Enable Discord Rich Presence."
					icon={<img className="w-6 invert-100" src={DiscordIcon} />}
					title="Discord RPC"
				>
					<Switch
						defaultSelected={discordRpc}
						onChange={val => setDiscordRpc(val)}
					/>
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
					description="Unknown for now"
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
