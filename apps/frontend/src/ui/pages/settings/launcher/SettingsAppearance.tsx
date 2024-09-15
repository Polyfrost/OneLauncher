import { useBeforeLeave } from '@solidjs/router';
import { Window } from '@tauri-apps/api/window';
import { Monitor01Icon, PackageIcon, Speedometer04Icon } from '@untitled-theme/icons-solid';
import { bridge } from '~imports';
import Dropdown from '~ui/components/base/Dropdown';
import Toggle from '~ui/components/base/Toggle';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Sidebar from '~ui/components/Sidebar';
import useSettings from '~ui/hooks/useSettings';
import { upperFirst } from '~utils';
import { BROWSER_VIEWS } from '~utils/browser';
import { createSignal, For } from 'solid-js';
import SettingsRow from '../../../components/SettingsRow';

function SettingsAppearance() {
	const { settings, saveOnLeave } = useSettings();
	const [shouldReload, setShouldReload] = createSignal(false);

	useBeforeLeave((e) => {
		if (shouldReload()) {
			e.preventDefault();
			setShouldReload(false);
			location.reload();
		}

		if (settings().custom_frame !== undefined)
			bridge.commands.setWindowStyle(settings().custom_frame!);
	});

	saveOnLeave(() => ({
		disable_animations: settings().disable_animations!,
		custom_frame: settings().custom_frame!,
	}));

	return (
		<Sidebar.Page>
			<h1>Appearance</h1>
			<ScrollableContainer>
				{/* <div class="flex flex-row items-center">
					<p>theme placeholder</p>
				</div> */}

				{/* <SettingsRow
					description="The main color used across the launcher. This doesn't edit your theme."
					icon={<PaintPourIcon />}
					title="Accent Color"
				>
					<Button iconLeft={<ColorsIcon />}>#ff0000</Button>
				</SettingsRow> */}

				<SettingsRow
					description="Change the look of the package list."
					icon={<PackageIcon />}
					title="Package List Style"
				>
					<Dropdown
						onChange={value => settings().browser_list_view = BROWSER_VIEWS[value] ?? 'grid'}
						selected={() => BROWSER_VIEWS.indexOf(settings().browser_list_view ?? 'grid')}
					>
						<For each={BROWSER_VIEWS}>
							{view => (
								<Dropdown.Row>{upperFirst(view)}</Dropdown.Row>
							)}
						</For>
					</Dropdown>
				</SettingsRow>

				<SettingsRow
					description="Uses custom window frame for the launcher."
					icon={<Monitor01Icon />}
					title="Custom Window Frame"
				>
					<Toggle
						checked={() => settings().custom_frame ?? true}
						onChecked={(value) => {
							settings().custom_frame = value;
							Window.getCurrent().setDecorations(value);
							// setShouldReload(true);
						}}
					/>
				</SettingsRow>

				<SettingsRow
					description="Disables all animations in the launcher."
					icon={<Speedometer04Icon />}
					title="Disable Animations"
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
