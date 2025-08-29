// import ScrollableContainer from '@/components/ScrollableContainer';
import SettingsRow from '@/components/SettingsRow';
import SettingsSwitch from '@/components/SettingSwitch';
import { useSettings } from '@/hooks/useSettings';
import { bindings } from '@/main';
import { THEMES } from '@/utils/theming';
import { Switch } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { Monitor01Icon, PackageIcon, Speedometer04Icon } from '@untitled-theme/icons-react';
import Sidebar from './route';

export const Route = createFileRoute('/app/settings/appearance')({
	component: RouteComponent,
});

function RouteComponent() {
	const { createSetting } = useSettings();

	return (
		<Sidebar.Page>
			{/* <ScrollableContainer> */}
			<div className="h-full">
				<h1>Appearance</h1>

				<div className="flex flex-row items-center gap-4">
					<PrimaryThemeCard />
					<div className="grid grid-cols-3 h-full gap-4">
						{/* <For each={Object.keys(THEMES) as (keyof typeof THEMES)[]}>
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
              </For> */}

						{Object.keys(THEMES).map((theme) => {
							return THEMES[theme as keyof typeof THEMES].variants.map((variant) => {
								return (
									<ThemeCard key={`${theme}-${variant.name}`} setTheme={() => { }} themeSelector={`${theme}-${variant.name}`} />
								);
							});
						})}
					</div>
				</div>

				<SettingsRow
					description="Change the look of the package list."
					icon={<PackageIcon />}
					title="Package List Style"
				>
					<p>i hope i'll fix this</p>
				</SettingsRow>

				<SettingsRow
					description="Uses custom window frame for the launcher."
					icon={<Monitor01Icon />}
					title="Custom Window Frame"
				>
					<SettingsSwitch setting={createSetting('native_window_frame')} />
				</SettingsRow>

				<SettingsRow
					description="Disables all animations in the launcher."
					icon={<Speedometer04Icon />}
					title="Disable Animations"
				>
					{/* <SettingsSwitch setting={createSetting("disable_animations")}/> */}
				</SettingsRow>
			</div>
			{/* </ScrollableContainer> */}
		</Sidebar.Page>
	);
}

interface ThemeCardProps {
	themeSelector: string;
	setTheme: (merged: string) => void;
};

function ThemeCard({
	themeSelector,
	setTheme,
}: ThemeCardProps) {
	return (
		<div className={`h-min theme-${themeSelector}`} onClick={() => setTheme(themeSelector)}>
			<svg
				fill="none"
				height="78"
				viewBox="0 0 126 78"
				width="126"
				xmlns="http://www.w3.org/2000/svg"
			>
				<rect
					className="fill-page-elevated"
					height="78"
					rx="8"
					width="126"
				/>
				<rect
					className="stroke-border"
					height="77"
					rx="7.5"
					stroke-opacity="0.1"
					width="125"
					x="0.5"
					y="0.5"
				/>
				<path
					className="stroke-fg-primary"
					d="M8 16H116"
					stroke-linecap="round"
					stroke-width="5"
				/>
				<path
					className="stroke-fg-secondary"
					d="M8 26H56"
					stroke-linecap="round"
					stroke-width="3"
				/>
				<rect
					className="fill-brand"
					fill-opacity="0.5"
					height="16"
					rx="4"
					width="64"
					x="54"
					y="54"
				/>
				<path d="M54 58C54 55.7909 55.7909 54 58 54H86V70H58C55.7909 70 54 68.2091 54 66V58Z" fill="rgb(var(--clr-brand))" />
			</svg>

		</div>
	);
}

function PrimaryThemeCard() {
	return (
		<div className="w-[296] h-[183]">
			<svg
				fill="none"
				height="183"
				viewBox="0 0 296 183"
				width="296"
				xmlns="http://www.w3.org/2000/svg"
			>
				<rect
					className="fill-page"
					height="183"
					rx="16"
					width="296"
				/>
				<rect
					className="stroke-border/10"
					height="180"
					rx="14.5"
					strokeWidth="1.5"
					width="293"
					x="1.5"
					y="1.5"
				/>
				<rect
					className="fill-page-elevated"
					height="123"
					rx="8"
					width="168"
					x="112"
					y="44"
				/>
				<rect
					className="stroke-border/5"
					height="122"
					rx="7.5"
					width="167"
					x="112.5"
					y="44.5"
				/>
				<rect
					className="fill-brand/50"
					height="16"
					rx="4"
					width="64"
					x="208"
					y="143"
				/>
				<path
					className="stroke-fg-primary"
					d="M132 64H240"
					strokeLinecap="round"
					strokeWidth="5"
				/>
				<path
					className="stroke-fg-secondary"
					d="M132 74H180"
					strokeLinecap="round"
					strokeWidth="3"
				/>
				<path
					className="stroke-code-error"
					d="M132 94H216"
					strokeLinecap="round"
					strokeWidth="4"
				/>
				<path
					className="stroke-code-warn"
					d="M132 104H192"
					strokeLinecap="round"
					strokeWidth="4"
				/>
				<path
					className="stroke-code-info"
					d="M132 114H210"
					strokeLinecap="round"
					strokeWidth="4"
				/>
				<path className="fill-brand" d="M208 147C208 144.791 209.791 143 212 143H240V159H212C209.791 159 208 157.209 208 155V147Z" />
			</svg>
		</div>
	);
}
