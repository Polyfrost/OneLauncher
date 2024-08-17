import { AlertSquareIcon, FolderIcon, LinkExternal01Icon, XIcon } from '@untitled-theme/icons-solid';
import { open } from '@tauri-apps/plugin-shell';
import SettingsRow from '../../../components/SettingsRow';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Toggle from '~ui/components/base/Toggle';
import Sidebar from '~ui/components/Sidebar';
import useSettingsContext from '~ui/hooks/useSettings';
import Button from '~ui/components/base/Button';
import DiscordIcon from '~assets/logos/discord.svg?component-solid';

function SettingsGeneral() {
	const { settings, saveOnLeave } = useSettingsContext();

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
					title="Discord RPC"
					description="Enable Discord Rich Presence."
					icon={<DiscordIcon class="w-6" />}
				>
					<Toggle
						checked={() => !(settings().disable_discord ?? false)}
						onChecked={value => settings().disable_discord = !value}
					/>
				</SettingsRow>

				<SettingsRow
					title="Hide Close Dialog"
					description="Hide the confirmation dialog when closing the launcher."
					icon={<XIcon />}
				>
					<Toggle
						checked={() => settings().hide_close_prompt ?? true}
						onChecked={value => settings().hide_close_prompt = value}
					/>
				</SettingsRow>

				<SettingsRow
					title="Error Analytics"
					description="Sends errors and crash logs using Sentry to help developers fix issues. (// TODO)"
					icon={<AlertSquareIcon />}
				>
					<Toggle
						checked={() => !(settings().disable_analytics ?? false)}
						onChecked={value => settings().disable_analytics = !value}
					/>
				</SettingsRow>

				<SettingsRow.Header>Folders and Files</SettingsRow.Header>
				<SettingsRow
					title="Launcher Folder"
					description={settings().config_dir || 'Unknown'}
					icon={<FolderIcon />}
				>
					<Button
						iconLeft={<LinkExternal01Icon />}
						children="Open"
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
