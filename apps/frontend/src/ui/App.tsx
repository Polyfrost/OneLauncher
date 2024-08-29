import type { ParentProps } from 'solid-js';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import { PROGRAM_INFO } from '@onelauncher/client/bindings';
import WindowFrame from './components/WindowFrame';
import Navbar from './components/Navbar';
import ErrorBoundary from './components/ErrorBoundary';
import AnimatedRoutes from './components/AnimatedRoutes';
import NotificationOverlay from './components/overlay/notifications/NotificationOverlay';
import { SettingsProvider } from './hooks/useSettings';
import { ClusterModalControllerProvider } from './components/overlay/cluster/ClusterCreationModal';
import { MultiProvider } from './components/MultiProvider';
import { AccountControllerProvider } from './components/overlay/account/AddAccountModal';
import { ModalProvider, ModalRenderer } from './components/overlay/Modal';
import { BrowserProvider } from './hooks/useBrowser';

function App(props: ParentProps) {
	if (PROGRAM_INFO.dev_build !== true)
		document.addEventListener('contextmenu', e => e.preventDefault());

	return (
		<GlobalContexts>
			<main class="h-screen max-h-screen min-h-screen w-full flex flex-col overflow-hidden bg-page text-fg-primary">
				<WindowFrame />
				<div class="flex flex-col px-8">
					<Navbar />
				</div>

				<div class="relative h-full w-full overflow-hidden">
					<div class="absolute left-0 top-0 h-full w-full flex flex-col overflow-x-hidden">
						<ErrorBoundary>
							<OverlayScrollbarsComponent class="os-hide-horizontal-scrollbar absolute left-0 top-0 h-full w-full flex flex-col overflow-x-hidden overflow-y-auto px-8 pb-8">
								<div class="h-full flex-1">
									<AnimatedRoutes>
										{props.children}
									</AnimatedRoutes>
								</div>
							</OverlayScrollbarsComponent>
						</ErrorBoundary>
					</div>
				</div>

				<NotificationOverlay />
				<ModalRenderer />
			</main>
		</GlobalContexts>
	);
}

export default App;

function GlobalContexts(props: ParentProps) {
	return (
		<MultiProvider
			values={[
				ModalProvider,
				SettingsProvider,
				AccountControllerProvider,
				ClusterModalControllerProvider,
				BrowserProvider,
			]}
		>
			{props.children}
		</MultiProvider>
	);
}
