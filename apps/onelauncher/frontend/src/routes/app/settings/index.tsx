import DiscordIcon from '@/assets/logos/discord.svg';
import SettingsRow from '@/components/SettingsRow';
import { bindings } from '@/main';
import { useCommand } from '@onelauncher/common';
import { Button, Switch } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { FolderIcon, LinkExternal01Icon, XIcon } from '@untitled-theme/icons-react';
import Sidebar from './route';

export const Route = createFileRoute('/app/settings/')({
	component: RouteComponent,
});

function RouteComponent() {
	const result = useCommand('getGlobalProfile', bindings.core.getGlobalProfile);

	return (
		<Sidebar.Page>
			<div className="h-full">
				<h1>General Settings</h1>

				<SettingsRow
					description="Enable Discord Rich Presence."
					icon={<img className="w-6 invert-100" src={DiscordIcon} />}
					title="Discord RPC"
				>
					<Switch />
				</SettingsRow>

				<SettingsRow
					description="Hide the confirmation dialog when closing the launcher."
					icon={<XIcon />}
					title="Hide Close Dialog"
				>
					<Switch />
				</SettingsRow>

				{/* <SettingsRow
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
					<Button size="normal">
						<LinkExternal01Icon />
						{' '}
						Open
					</Button>
				</SettingsRow>
			</div>
		</Sidebar.Page>
	);
}
