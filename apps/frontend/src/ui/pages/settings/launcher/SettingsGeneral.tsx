import { open } from '@tauri-apps/plugin-shell';
import { FolderIcon, LinkExternal01Icon, XIcon } from '@untitled-theme/icons-solid';
import DiscordIcon from '~assets/logos/discord.svg?component-solid';
import Button from '~ui/components/base/Button';
import Toggle from '~ui/components/base/Toggle';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Sidebar from '~ui/components/Sidebar';
import useSettings from '~ui/hooks/useSettings';
import SettingsRow from '../../../components/SettingsRow';

function SettingsGeneral() {
	const { settings, saveOnLeave } = useSettings();

	saveOnLeave(() => ({
		disable_discord: settings().disable_discord!,
		hide_close_prompt: settings().hide_close_prompt!,
		disable_analytics: settings().disable_analytics!,
	}));

	return (
		<Sidebar.Page>
			<h1>General</h1>
			<ScrollableContainer>

				<SettingsRow
					description="Enable Discord Rich Presence."
					icon={<DiscordIcon class="w-6" />}
					title="Discord RPC"
				>
					<Toggle
						checked={() => !(settings().disable_discord ?? false)}
						onChecked={value => settings().disable_discord = !value}
					/>
				</SettingsRow>

				<SettingsRow
					description="Hide the confirmation dialog when closing the launcher."
					icon={<XIcon />}
					title="Hide Close Dialog"
				>
					<Toggle
						checked={() => settings().hide_close_prompt ?? true}
						onChecked={value => settings().hide_close_prompt = value}
					/>
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
					description={settings().config_dir || 'Unknown'}
					icon={<FolderIcon />}
					title="Launcher Folder"
				>
					<Button
						children="Open"
						iconLeft={<LinkExternal01Icon />}
						onClick={() => {
							if (settings().config_dir)
								open(settings().config_dir!);
						}}
					/>
				</SettingsRow>

			</ScrollableContainer>
		</Sidebar.Page>
	);
}

export default SettingsGeneral;
