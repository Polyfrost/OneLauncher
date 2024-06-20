import { EyeIcon, LayoutTopIcon, Maximize01Icon, XIcon } from '@untitled-theme/icons-solid';
import Dropdown from '~ui/components/base/Dropdown';
import TextField from '~ui/components/base/TextField';
import Toggle from '~ui/components/base/Toggle';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import SettingsRow from '~ui/components/SettingsRow';
import Sidebar from '~ui/components/Sidebar';
import useSettingsContext from '~ui/hooks/useSettings';

function SettingsMinecraft() {
	const settings = useSettingsContext();

	return (
		<Sidebar.Page>
			<h1>Global Minecraft Settings</h1>
			<ScrollableContainer>

				<SettingsRow
					title="Force Fullscreen"
					description="Force Minecraft to start in fullscreen mode."
					icon={<Maximize01Icon />}
				>
					<Toggle
						defaultChecked={settings.force_fullscreen ?? false}
						onChecked={value => settings.force_fullscreen = value}
					/>
				</SettingsRow>

				<SettingsRow
					title="Resolution"
					description="The game window resolution in pixels."
					icon={<LayoutTopIcon />}
				>
					<div class="grid grid-justify-center grid-items-center gap-2 grid-cols-[70px_16px_70px]">
						<TextField.Number
							class="text-center"
							value={settings.resolution[0]}
							onValidSubmit={(value) => {
								settings.resolution = [Number.parseInt(value), settings.resolution[1]];
							}}
						/>
						<XIcon class="w-4 h-4" />
						<TextField.Number
							class="text-center"
							value={settings.resolution[1]}
							onValidSubmit={(value) => {
								settings.resolution = [settings.resolution[0], Number.parseInt(value)];
							}}
						/>
					</div>
				</SettingsRow>

				<SettingsRow
					title="Launcher Visibility"
					description="Set the launcher visibility whenever you start a game."
					icon={<EyeIcon />}
				>
					<Dropdown
						class="w-32!"
						onChange={(selected) => {
							switch (selected) {
								case 0:
									settings.hide_on_launch = false;
									break;
								case 1:
									settings.hide_on_launch = true;
									break;
								case 2:
									break;
							}
						}}
					>
						<Dropdown.Row>Visible</Dropdown.Row>
						<Dropdown.Row>Hide</Dropdown.Row>
						<Dropdown.Row>Close (// TODO)</Dropdown.Row>
					</Dropdown>
				</SettingsRow>

			</ScrollableContainer>
		</Sidebar.Page>
	);
}

export default SettingsMinecraft;
