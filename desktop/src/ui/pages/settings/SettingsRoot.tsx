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
import { AppInfo } from '~utils/program-info';

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
		<div class="flex flex-row flex-1 h-full gap-x-7">
			<div class="mt-8 flex flex-col justify-between">
				<Sidebar
					base="/settings"
					state={{}}
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

			<div class="flex flex-col w-full h-full">
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
		<div class="mx-1 my-2 flex flex-col justify-center items-center relative">
			<Tooltip text={copied() ? 'Copied to clipboard!' : 'Copy to clipboard'}>
				<div
					class="whitespace-pre-line line-height-normal text-sm text-fg-secondary"
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
Launcher Version: ${AppInfo.launcher_version}
Tauri Version: ${AppInfo.tauri_version}
Webview Version: ${AppInfo.webview_version} 
Platform: ${AppInfo.platform} ${AppInfo.arch} bit
Build: ${AppInfo.dev_build ? 'dev' : 'release'}
`.trim()}
				</div>
			</Tooltip>
		</div>
	);
}

SettingsRoot.Routes = SettingsRoutes;

export default SettingsRoot;
