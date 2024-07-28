import { ColorsIcon, PaintPourIcon, Speedometer04Icon } from '@untitled-theme/icons-solid';
import SettingsRow from '../../../components/SettingsRow';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Button from '~ui/components/base/Button';
import Toggle from '~ui/components/base/Toggle';
import Sidebar from '~ui/components/Sidebar';
import useSettingsContext from '~ui/hooks/useSettings';

function SettingsAppearance() {
	const settings = useSettingsContext();

	return (
		<Sidebar.Page>
			<h1>Appearance</h1>
			<ScrollableContainer>
				<div class="flex flex-row items-center">
					<p>theme placeholder</p>
				</div>

				<SettingsRow
					title="Accent Color"
					description="The main color used across the launcher. This doesn't edit your theme."
					icon={<PaintPourIcon />}
				>
					<Button iconLeft={<ColorsIcon />}>#ff0000</Button>
				</SettingsRow>

				<SettingsRow
					title="Disable Animations"
					description="Disables all animations in the launcher."
					icon={<Speedometer04Icon />}
				>
					<Toggle
						defaultChecked={settings.disable_animations ?? false}
						onChecked={value => settings.disable_animations = value}
					/>
				</SettingsRow>
			</ScrollableContainer>
		</Sidebar.Page>
	);
}

export default SettingsAppearance;
