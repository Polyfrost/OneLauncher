import { ColorsIcon, PaintPourIcon } from '@untitled-theme/icons-solid';
import SettingsRow from '../components';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Button from '~ui/components/base/Button';

function SettingsAppearance() {
	return (
		<ScrollableContainer title="Appearance">
			<div class="flex flex-row items-center flex-1">
				<p>theme placeholder</p>
			</div>
			<SettingsRow
				title="Accent Color"
				description="The main color used across the launcher. This doesn’t edit your theme."
				icon={<PaintPourIcon />}
			>
				<Button iconLeft={<ColorsIcon />}>#ff0000</Button>
			</SettingsRow>
		</ScrollableContainer>
	);
}

export default SettingsAppearance;
