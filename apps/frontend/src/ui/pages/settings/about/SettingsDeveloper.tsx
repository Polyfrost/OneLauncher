import { useNavigate } from '@solidjs/router';
import { CodeBrowserIcon, EyeIcon, GitMergeIcon, LinkExternal01Icon, RefreshCcw05Icon } from '@untitled-theme/icons-solid';
import Button from '~ui/components/base/Button';
import Toggle from '~ui/components/base/Toggle';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import SettingsRow from '~ui/components/SettingsRow';
import Sidebar from '~ui/components/Sidebar';
import useSettings from '~ui/hooks/useSettings';

function SettingsDeveloper() {
	const { settings, saveOnLeave } = useSettings();
	const navigate = useNavigate();

	saveOnLeave(() => ({
		debug_mode: settings().debug_mode!,
		onboarding_completed: settings().onboarding_completed!,
	}));

	return (
		<Sidebar.Page>
			<h1>Developer Options</h1>
			<p class="mb-2">You probably shouldn't mess with any of these if you don't know what you're doing!</p>
			<ScrollableContainer>
				<SettingsRow
					description="Opens the browser developer tools."
					icon={<CodeBrowserIcon />}
					title="Open Dev Tools"
				>
					<Button
						children="Open"
						iconLeft={<LinkExternal01Icon />}
					/>
				</SettingsRow>

				<SettingsRow
					description="Enables debug mode."
					icon={<GitMergeIcon />}
					title="Debug mode"
				>
					<Toggle
						checked={() => settings().debug_mode ?? false}
						onChecked={value => settings().debug_mode = value}
					/>
				</SettingsRow>

				<SettingsRow
					description="Reloads the launcher frontend."
					icon={<RefreshCcw05Icon />}
					title="Reload"
				>
					<Button
						children="Reload"
						iconLeft={<RefreshCcw05Icon />}
						onClick={() => location.reload()}
					/>
				</SettingsRow>

				<SettingsRow
					description="Enter onboarding mode"
					icon={<EyeIcon />}
					title="Onboarding"
				>
					<Button
						children="Open"
						iconLeft={<EyeIcon />}
						onClick={() => {
							settings().onboarding_completed = false;
							navigate('/onboarding');
						}}
					/>
				</SettingsRow>

			</ScrollableContainer>
		</Sidebar.Page>
	);
}

export default SettingsDeveloper;
