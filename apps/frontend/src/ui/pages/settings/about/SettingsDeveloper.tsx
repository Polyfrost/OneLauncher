import { CodeBrowserIcon, GitMergeIcon, LinkExternal01Icon, RefreshCcw05Icon } from '@untitled-theme/icons-solid';
import Button from '~ui/components/base/Button';
import Toggle from '~ui/components/base/Toggle';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import SettingsRow from '~ui/components/SettingsRow';
import Sidebar from '~ui/components/Sidebar';
import useSettingsContext from '~ui/hooks/useSettings';

function SettingsDeveloper() {
	const { settings } = useSettingsContext();

	return (
		<Sidebar.Page>
			<h1>Developer Options</h1>
			<p class="mb-2">You probably shouldn't mess with any of these if you don't know what you're doing!</p>
			<ScrollableContainer>
				<SettingsRow
					title="Open Dev Tools"
					description="Opens the browser developer tools."
					icon={<CodeBrowserIcon />}
				>
					<Button
						iconLeft={<LinkExternal01Icon />}
						children="Open"
					/>
				</SettingsRow>

				<SettingsRow
					title="Debug mode"
					description="Enables debug mode."
					icon={<GitMergeIcon />}
				>
					<Toggle
						checked={() => settings().debug_mode ?? false}
						onChecked={value => settings().debug_mode = value}
					/>
				</SettingsRow>

				<SettingsRow
					title="Reload"
					description="Reloads the launcher frontend."
					icon={<RefreshCcw05Icon />}
				>
					<Button
						iconLeft={<RefreshCcw05Icon />}
						onClick={() => location.href = '/'}
						children="Reload"
					/>
				</SettingsRow>

			</ScrollableContainer>
		</Sidebar.Page>
	);
}

export default SettingsDeveloper;
