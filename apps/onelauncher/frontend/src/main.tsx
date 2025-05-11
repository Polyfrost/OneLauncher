import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { createRouter, RouterProvider } from '@tanstack/react-router';
import { StrictMode } from 'react';
import ReactDOM from 'react-dom/client';
import { routeTree } from './routeTree.gen';

import './fonts';
import './utils/nativeExperience';
import './utils/devExperience';
import './styles/global.css';

export * as bindings from './bindings.gen';

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
