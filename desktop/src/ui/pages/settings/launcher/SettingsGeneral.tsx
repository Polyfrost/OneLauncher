import { XIcon } from '@untitled-theme/icons-solid';
import SettingsRow from '../components';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Toggle from '~ui/components/base/Toggle';
import appSettings from '~ui/state/appSettings';

function SettingsGeneral() {
	return (
		<ScrollableContainer title="General">
			<SettingsRow
                title='Show Close Dialog'
                description='Show a confirmation dialog when closing the launcher.'
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
	);
}

export default SettingsGeneral;
