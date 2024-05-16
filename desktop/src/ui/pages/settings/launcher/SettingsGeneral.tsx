import { Code01Icon } from '@untitled-theme/icons-solid';
import SettingsRow from '../components';
import ScrollableContainer from '~ui/components/ScrollableContainer';

function SettingsGeneral() {
	return (
		<ScrollableContainer title="General">
			<SettingsRow title="Title" description="In publishing and graphic design, Lorem ipsum is a placeholder text commonly used to demonstrate the" icon={<Code01Icon />} />
		</ScrollableContainer>
	);
}

export default SettingsGeneral;
