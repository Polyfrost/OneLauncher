import { LayoutTopIcon, Maximize01Icon, XIcon } from '@untitled-theme/icons-solid';
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
			<h1>Global Game Settings</h1>
			<ScrollableContainer>

				<SettingsRow
					title="Force Fullscreen"
					description="Force Minecraft to start in fullscreen mode"
					icon={<Maximize01Icon />}
				>
					<Toggle
						defaultChecked={settings.force_fullscreen ?? false}
						onChecked={value => settings.force_fullscreen = value}
					/>
				</SettingsRow>

				<SettingsRow
					title="Resolution"
					description="The game window resolution"
					icon={<LayoutTopIcon />}
				>
					<div class="grid grid-justify-center grid-items-center grid-cols-[70px_24px_70px]">
						<TextField.Number
							value={settings.resolution[0]}
							onValidSubmit={(value) => {
								settings.resolution = [Number.parseInt(value), settings.resolution[1]];
							}}
						/>
						<XIcon />
						<TextField.Number
							onValidSubmit={(value) => {
								settings.resolution = [settings.resolution[0], Number.parseInt(value)];
							}}
						/>
					</div>
				</SettingsRow>

			</ScrollableContainer>
		</Sidebar.Page>
	);
}

export default SettingsMinecraft;
