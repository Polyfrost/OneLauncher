import { isDev, registerDevExperience, registerNativeExperience } from '@onelauncher/common';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createRouter, RouterProvider } from '@tanstack/react-router';
import { StrictMode } from 'react';
import ReactDOM from 'react-dom/client';
import { createTauRPCProxy } from './bindings.gen';
import { NotificationProvider } from './hooks/useNotification';

import { routeTree } from './routeTree.gen';
import './fonts';
import 'overlayscrollbars/overlayscrollbars.css';
import './styles/global.css';

// Tauri bindings
export const bindings = createTauRPCProxy();

// Tanstack Query Client
const queryClient = new QueryClient({
	defaultOptions: {
		queries: {
			retry: false,
			enabled: false,
		},
	},
});

// Tanstack Router
const router = createRouter({
	routeTree,
	context: {
		queryClient,
	},
	defaultPreload: 'intent',
	scrollRestoration: true,
	defaultStructuralSharing: true,
	defaultPreloadStaleTime: 0,
	defaultNotFoundComponent: () => <div>Not Found</div>,
});

declare module '@tanstack/react-router' {
	interface Register {
		router: typeof router;
	}
}

// Register UX changes
if (isDev)
	registerDevExperience();

registerNativeExperience();

// Render the app
const rootElement = document.getElementById('app');
if (rootElement && !rootElement.innerHTML) {
	const root = ReactDOM.createRoot(rootElement);
	root.render(
		<StrictMode>
			<QueryClientProvider client={queryClient}>
				<NotificationProvider>
					<RouterProvider router={router} />
				</NotificationProvider>
			</QueryClientProvider>
		</StrictMode>,
	);
}
