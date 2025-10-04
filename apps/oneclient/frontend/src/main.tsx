import { isDev, registerDevExperience, registerNativeExperience } from '@onelauncher/common';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createHashHistory, createRouter, RouterProvider } from '@tanstack/react-router';
import { StrictMode } from 'react';
import ReactDOM from 'react-dom/client';
import { createTauRPCProxy } from './bindings.gen';
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
			staleTime: 1000 * 60 * 1, // 1 minute
		},
	},
});

const hashHistory = createHashHistory();

// Tanstack Router
const router = createRouter({
	routeTree,
	context: {
		queryClient,
	},
	history: hashHistory,
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
				<RouterProvider router={router} />
			</QueryClientProvider>
		</StrictMode>,
	);
}
