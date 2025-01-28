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
import { DEFAULT_THEME, setAppTheme, splitMergedTheme, THEMES } from '~utils/theming';
import { createEffect, createSignal, For } from 'solid-js';
import SettingsRow from '../../../components/SettingsRow';

function SettingsAppearance() {
	const { settings, saveOnLeave } = useSettings();
	const [shouldReload, setShouldReload] = createSignal(false);
	const [theme, setTheme] = createSignal(settings().theme ?? DEFAULT_THEME);

	createEffect(() => {
		document.body.classList.add('theme-transition');
		const split = splitMergedTheme(theme());
		setAppTheme(split.theme, split.variant);
		setTimeout(() => document.body.classList.remove('theme-transition'), 300);
	});

	useBeforeLeave((e) => {
		if (shouldReload()) {
			e.preventDefault();
			setShouldReload(false);
			location.reload();
		}

		if (settings().custom_frame !== undefined)
			bridge.commands.setWindowStyle(settings().custom_frame!);
	});

	// eslint-disable-next-line solid/reactivity -- This is a side effect
	saveOnLeave(() => ({
		disable_animations: settings().disable_animations!,
		custom_frame: settings().custom_frame!,
		theme: theme(),
	}));

	return (
		<Sidebar.Page>
			<h1>Appearance</h1>
			<ScrollableContainer>
				<div class="flex flex-row items-center gap-4">
					<PrimaryThemeCard merged={theme()} />
					<div class="grid grid-cols-3 h-full gap-4">
						<For each={Object.keys(THEMES) as (keyof typeof THEMES)[]}>
							{theme => (
								<For each={THEMES[theme ?? 'OneLauncher'].variants}>
									{variant => (
										<ThemeCard
											setTheme={setTheme}
											themeSelector={`${theme}-${variant.name}`}
										/>
									)}
								</For>
							)}
						</For>
					</div>
				</div>

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

interface ThemeCardProps {
	themeSelector: string;
	setTheme: (merged: string) => void;
};

function ThemeCard(props: ThemeCardProps) {
	return (
		<div class={`theme-${props.themeSelector}`} onClick={() => props.setTheme(props.themeSelector)}>
			<svg fill="none" height="78" viewBox="0 0 126 78" width="126" xmlns="http://www.w3.org/2000/svg">
				<rect fill="rgb(var(--clr-page))" height="78" rx="8" width="126" />
				<rect height="77" rx="7.5" stroke="rgb(var(--clr-border))" stroke-opacity="0.1" width="125" x="0.5" y="0.5" />
				<path d="M8 16H116" stroke="rgb(var(--clr-fg-primary))" stroke-linecap="round" stroke-width="5" />
				<path d="M8 26H56" stroke="rgb(var(--clr-fg-secondary))" stroke-linecap="round" stroke-width="3" />
				<rect fill="rgb(var(--clr-brand))" fill-opacity="0.5" height="16" rx="4" width="64" x="54" y="54" />
				<path d="M54 58C54 55.7909 55.7909 54 58 54H86V70H58C55.7909 70 54 68.2091 54 66V58Z" fill="rgb(var(--clr-brand))" />
			</svg>

		</div>
	);
}

interface PrimaryThemeCardProps {
	merged: string;
}

function PrimaryThemeCard(props: PrimaryThemeCardProps) {
	return (
		<div class={`theme-${props.merged}`}>
			<svg fill="none" height="183" viewBox="0 0 296 183" width="296" xmlns="http://www.w3.org/2000/svg">
				<rect fill="rgb(var(--clr-page))" height="183" rx="16" width="296" />
				<rect height="180" rx="14.5" stroke="rgb(var(--clr-border))" stroke-opacity="0.1" stroke-width="1.5" width="293" x="1.5" y="1.5" />
				<rect fill="rgb(var(--clr-page-elevated))" height="123" rx="8" width="168" x="112" y="44" />
				<rect height="122" rx="7.5" stroke="rgb(var(--clr-border))" stroke-opacity="0.05" width="167" x="112.5" y="44.5" />
				<rect fill="rgb(var(--clr-brand))" fill-opacity="0.5" height="16" rx="4" width="64" x="208" y="143" />
				<path d="M132 64H240" stroke="rgb(var(--clr-fg-primary))" stroke-linecap="round" stroke-width="5" />
				<path d="M132 74H180" stroke="rgb(var(--clr-fg-secondary))" stroke-linecap="round" stroke-width="3" />
				<path d="M132 94H216" stroke="rgb(var(--clr-code-error))" stroke-linecap="round" stroke-width="4" />
				<path d="M132 104H192" stroke="rgb(var(--clr-code-warn))" stroke-linecap="round" stroke-width="4" />
				<path d="M132 114H210" stroke="rgb(var(--clr-code-info))" stroke-linecap="round" stroke-width="4" />
				<path d="M208 147C208 144.791 209.791 143 212 143H240V159H212C209.791 159 208 157.209 208 155V147Z" fill="rgb(var(--clr-brand))" />
			</svg>

		</div>
	);
}
