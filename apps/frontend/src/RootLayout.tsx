import { getProgramInfo } from '@onelauncher/client';
import AnimatedRoutes from '~ui/components/AnimatedRoutes';
import { onMount, type ParentProps } from 'solid-js';
import { MultiProvider } from './ui/components/MultiProvider';
import { AccountControllerProvider } from './ui/components/overlay/account/AddAccountModal';
import { ClusterModalControllerProvider } from './ui/components/overlay/cluster/ClusterCreationModal';
import { ModalProvider, ModalRenderer } from './ui/components/overlay/Modal';
import NotificationOverlay from './ui/components/overlay/notifications/NotificationOverlay';
import WindowFrame from './ui/components/WindowFrame';
import { BrowserProvider } from './ui/hooks/useBrowser';
import { SettingsProvider } from './ui/hooks/useSettings';

function RootLayout(props: ParentProps) {
	onMount(() => {
		if (getProgramInfo().dev_build !== true)
			document.addEventListener('contextmenu', e => e.preventDefault());
	});

	return (
		<GlobalContexts>
			<main class="h-screen max-h-screen min-h-screen w-full flex flex-col overflow-hidden bg-page text-fg-primary">
				<WindowFrame />

				<AnimatedRoutes animation="fade" appear>
					{props.children}
				</AnimatedRoutes>

				<NotificationOverlay />
				<ModalRenderer />
			</main>
		</GlobalContexts>
	);
}

export default RootLayout;

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
