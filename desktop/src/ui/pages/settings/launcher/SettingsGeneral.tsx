import { XIcon } from '@untitled-theme/icons-solid';
import { createEffect } from 'solid-js';
import SettingsRow from '../../../components/SettingsRow';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Toggle from '~ui/components/base/Toggle';
import Sidebar from '~ui/components/Sidebar';
import { bridge } from '~index';
import useCommand from '~ui/hooks/useCommand';
import useSettingsContext from '~ui/hooks/useSettings';

function SettingsGeneral() {
	const settings = useSettingsContext();

	createEffect(() => {
		console.log(settings);
	});

	return (
		<Sidebar.Page>
			<h1>General</h1>
			<ScrollableContainer>
				<SettingsRow
					title="Show Close Dialog"
					description="Show a confirmation dialog when closing the launcher."
					icon={<XIcon />}
				>
					<Toggle />
				</SettingsRow>
			</ScrollableContainer>
		</Sidebar.Page>
	);
}

export default SettingsGeneral;
