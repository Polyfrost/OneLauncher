import { XIcon } from '@untitled-theme/icons-solid';
import SettingsRow from '../components';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Toggle from '~ui/components/base/Toggle';
import appSettings from '~ui/state/appSettings';
import Sidebar from '~ui/components/Sidebar';

function SettingsGeneral() {
	return (
		<Sidebar.Page>
			<h1>General</h1>
			<ScrollableContainer>
				<SettingsRow
					title="Show Close Dialog"
					description="Show a confirmation dialog when closing the launcher."
					icon={<XIcon />}
				>
					<Toggle
						defaultChecked={appSettings.settings.closeDialog}
						onChecked={(checked) => {
							appSettings.setSettings('closeDialog', checked);
						}}
					/>
				</SettingsRow>
			</ScrollableContainer>
		</Sidebar.Page>
	);
}

export default SettingsGeneral;
