import { PROGRAM_INFO } from '@onelauncher/client/bindings';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import type { ParentProps } from 'solid-js';
import AnimatedRoutes from './components/AnimatedRoutes';
import ErrorBoundary from './components/ErrorBoundary';
import { MultiProvider } from './components/MultiProvider';
import Navbar from './components/Navbar';
import { AccountControllerProvider } from './components/overlay/account/AddAccountModal';
import { ClusterModalControllerProvider } from './components/overlay/cluster/ClusterCreationModal';
import { ModalProvider, ModalRenderer } from './components/overlay/Modal';
import NotificationOverlay from './components/overlay/notifications/NotificationOverlay';
import WindowFrame from './components/WindowFrame';
import { BrowserProvider } from './hooks/useBrowser';
import { SettingsProvider } from './hooks/useSettings';

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
