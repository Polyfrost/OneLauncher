import { type ParentProps, createSignal } from 'solid-js';
import { Brush01Icon, CodeSnippet02Icon, MessageTextSquare01Icon, RefreshCcw02Icon, Rocket02Icon, Sliders04Icon, Users01Icon } from '@untitled-theme/icons-solid';
import { Route } from '@solidjs/router';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import Sidebar from '../../components/Sidebar';
import SettingsGeneral from './launcher/SettingsGeneral';
import SettingsAppearance from './launcher/SettingsAppearance';
import SettingsAccounts from './game/SettingsAccounts';
import SettingsMinecraft from './game/SettingsMinecraft';
import SettingsDeveloper from './about/SettingsDeveloper';
import SettingsFeedback from './about/SettingsFeedback';
import SettingsChangelog from './about/SettingsChangelog';
import AnimatedRoutes from '~ui/components/AnimatedRoutes';
import ErrorBoundary from '~ui/components/ErrorBoundary';
import Tooltip from '~ui/components/base/Tooltip';
import { PROGRAM_INFO } from '~bindings';

function SettingsRoutes() {
	return (
		<>
			{/* Launcher Settings */}
			<Route path="/" component={SettingsGeneral} />
			<Route path="/appearance" component={SettingsAppearance} />

			{/* Game Settings */}
			<Route path="/minecraft" component={SettingsMinecraft} />
			<Route path="/accounts" component={SettingsAccounts} />

			{/* About */}
			<Route path="/feedback" component={SettingsFeedback} />
			<Route path="/changelog" component={SettingsChangelog} />
			<Route path="/developer" component={SettingsDeveloper} />
		</>
	);
}

function SettingsRoot(props: ParentProps) {
	return (
		<div class="h-full flex flex-1 flex-row gap-x-7">
			<div class="mt-8 flex flex-col justify-between">
				<Sidebar
					base="/settings"
					links={{
						'Launcher Settings': [
							[<Rocket02Icon />, 'General', '/'],
							[<Brush01Icon />, 'Appearance', '/appearance'],
							// [<Key01Icon />, 'APIs', '/apis'],
							// [<Globe01Icon />, 'Language', '/language'],
						],
						'Game Settings': [
							[<Sliders04Icon />, 'Minecraft settings', '/minecraft'],
							[<Users01Icon />, 'Accounts', '/accounts'],
						],
						'About': [
							[<RefreshCcw02Icon />, 'Changelog', '/changelog'],
							[<MessageTextSquare01Icon />, 'Feedback', '/feedback'],
							[<CodeSnippet02Icon />, 'Developer Options', '/developer'],
						],
					}}
				/>
				<Info />
			</div>

			<div class="h-full w-full flex flex-col">
				<AnimatedRoutes>
					<ErrorBoundary>
						{props.children}
					</ErrorBoundary>
				</AnimatedRoutes>
			</div>
		</div>
	);
}

function Info() {
	const [copied, setCopied] = createSignal(false);

	return (
		<div class="relative mx-1 my-2 flex flex-col items-center justify-center">
			<Tooltip text={copied() ? 'Copied to clipboard!' : 'Copy to clipboard'}>
				<div
					class="whitespace-pre-line text-sm text-fg-secondary line-height-normal"
					onClick={(e) => {
						const info = e.target.innerHTML;
						writeText(info)
							.finally(() => {
								setCopied(true);
								setTimeout(() => setCopied(false), 3000);
							});
					}}
				>
					{`
Launcher Version: ${PROGRAM_INFO.launcher_version}
Tauri Version: ${PROGRAM_INFO.tauri_version}
Webview Version: ${PROGRAM_INFO.webview_version}
Platform: ${PROGRAM_INFO.platform} ${PROGRAM_INFO.arch} bit
Build: ${PROGRAM_INFO.dev_build ? 'dev' : 'release'}
`.trim()}
				</div>
			</Tooltip>
		</div>
	);
}

SettingsRoot.Routes = SettingsRoutes;

export default SettingsRoot;
