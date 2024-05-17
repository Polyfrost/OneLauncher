import { ColorsIcon, PaintPourIcon, Speedometer04Icon } from '@untitled-theme/icons-solid';
import SettingsRow from '../components';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Button from '~ui/components/base/Button';
import Toggle from '~ui/components/base/Toggle';
import reducedMotionTracker from '~ui/state/reducedMotionTracker';

function SettingsAppearance() {
	const { reducedMotion, setReducedMotion } = reducedMotionTracker;

	return (
		<ScrollableContainer title="Appearance">
			<div class="flex flex-row items-center flex-1">
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
				title="Animations"
				description="Toggle all animations in the launcher."
				icon={<Speedometer04Icon />}
			>
				<Toggle
					defaultChecked={reducedMotion()}
					onChecked={(checked) => {
						setReducedMotion(checked);
					}}
				/>
			</SettingsRow>
		</ScrollableContainer>
	);
}

export default SettingsAppearance;
