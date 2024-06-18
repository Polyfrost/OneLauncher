import type { ParentProps } from 'solid-js';
import { Brush01Icon, CodeSnippet02Icon, Globe01Icon, Key01Icon, MessageTextSquare01Icon, RefreshCcw02Icon, Rocket02Icon, Sliders04Icon, Users01Icon } from '@untitled-theme/icons-solid';
import { Route } from '@solidjs/router';
import Sidebar from '../../components/Sidebar';
import SettingsGeneral from './launcher/SettingsGeneral';
import SettingsAppearance from './launcher/SettingsAppearance';
import SettingsAccounts from './game/SettingsAccounts';
import SettingsMinecraft from './game/SettingsMinecraft';
import AnimatedRoutes from '~ui/components/AnimatedRoutes';
import ErrorBoundary from '~ui/components/ErrorBoundary';

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
		</>
	);
}

function SettingsRoot(props: ParentProps) {
	return (
		<div class="flex flex-row flex-1 h-full gap-x-7">
			<div class="mt-8">
				<Sidebar
					base="/settings"
					state={{}}
					links={{
						'Launcher Settings': [
							[<Rocket02Icon />, 'General', '/'],
							[<Brush01Icon />, 'Appearance', '/appearance'],
							[<Key01Icon />, 'APIs', '/apis'],
							[<Globe01Icon />, 'Language', '/language'],
						],
						'Game Settings': [
							[<Sliders04Icon />, 'Minecraft settings', '/minecraft'],
							[<Users01Icon />, 'Accounts', '/accounts'],
						],
						'About': [
							[<CodeSnippet02Icon />, 'Developer Options', '/developer'],
							[<RefreshCcw02Icon />, 'Changelog', '/changelog'],
							[<MessageTextSquare01Icon />, 'Feedback', '/feedback'],
						],
					}}
				/>
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

SettingsRoot.Routes = SettingsRoutes;

export default SettingsRoot;
