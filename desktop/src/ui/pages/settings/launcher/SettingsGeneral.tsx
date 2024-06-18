import { XIcon } from '@untitled-theme/icons-solid';
import SettingsRow from '../../../components/SettingsRow';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Toggle from '~ui/components/base/Toggle';
import Sidebar from '~ui/components/Sidebar';
import useSettingsContext from '~ui/hooks/useSettings';

function SettingsGeneral() {
	const settings = useSettingsContext();

	return (
		<Sidebar.Page>
			<h1>General</h1>
			<ScrollableContainer>

				<SettingsRow
					title="Hide Close Dialog"
					description="Hide the close confirmation dialog when closing the launcher."
					icon={<XIcon />}
				>
					<Toggle
						defaultChecked={settings.hide_close_prompt ?? true}
						onChecked={value => settings.hide_close_prompt = value}
					/>
				</SettingsRow>

			</ScrollableContainer>
		</Sidebar.Page>
	);
}

export default SettingsGeneral;
