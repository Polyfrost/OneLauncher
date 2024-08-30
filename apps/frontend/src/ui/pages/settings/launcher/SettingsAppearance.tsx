import { ColorsIcon, PackageIcon, PaintPourIcon, Speedometer04Icon } from '@untitled-theme/icons-solid';
import { useBeforeLeave } from '@solidjs/router';
import { For, createSignal } from 'solid-js';
import SettingsRow from '../../../components/SettingsRow';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Button from '~ui/components/base/Button';
import Toggle from '~ui/components/base/Toggle';
import Sidebar from '~ui/components/Sidebar';
import useSettings from '~ui/hooks/useSettings';
import Dropdown from '~ui/components/base/Dropdown';
import { BROWSER_VIEWS } from '~utils/browser';

function SettingsAppearance() {
	const { settings, saveOnLeave } = useSettings();
	const [shouldReload, setShouldReload] = createSignal(false);

	useBeforeLeave((e) => {
		if (shouldReload()) {
			e.preventDefault();
			setShouldReload(false);
			location.reload();
		}
	});

	saveOnLeave(() => ({
		disable_animations: settings().disable_animations!,
	}));

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
					title="Package List Style"
					description="Change the look of the package list."
					icon={<PackageIcon />}
				>
					<Dropdown
						selected={() => BROWSER_VIEWS.indexOf(settings().browser_list_view ?? 'grid')}
						onChange={value => settings().browser_list_view = BROWSER_VIEWS[value] ?? 'grid'}
					>
						<For each={BROWSER_VIEWS}>
							{view => (
								<Dropdown.Row>{view}</Dropdown.Row>
							)}
						</For>
					</Dropdown>
				</SettingsRow>

				<SettingsRow
					title="Disable Animations"
					description="Disables all animations in the launcher."
					icon={<Speedometer04Icon />}
				>
					<Toggle
						checked={() => settings().disable_animations ?? false}
						onChecked={(value) => {
							settings().disable_animations = value;
							setShouldReload(true);
						}}
					/>
				</SettingsRow>
			</ScrollableContainer>
		</Sidebar.Page>
	);
}

export default SettingsAppearance;
