import type { QueryClient } from '@tanstack/react-query';
import type { NavigateOptions, ToOptions } from '@tanstack/react-router';
import { Toasts } from '@/components/overlay';
import { TanStackDevtools } from '@tanstack/react-devtools';
import { ReactQueryDevtoolsPanel } from '@tanstack/react-query-devtools';
import { createRootRouteWithContext, Outlet, useRouter } from '@tanstack/react-router';
import { TanStackRouterDevtoolsPanel } from '@tanstack/react-router-devtools';
import { RouterProvider } from 'react-aria-components';

interface AppRouterContext {
	queryClient: QueryClient;
}

declare module 'react-aria-components' {
	interface RouterConfig {
		href: ToOptions['to'];
		routerOptions: Omit<NavigateOptions, keyof ToOptions>;
	}
}

export const Route = createRootRouteWithContext<AppRouterContext>()({
	component: RootRoute,
});

function RootRoute() {
	const router = useRouter();
	return (
		<RouterProvider
			navigate={(to, options) => router.navigate({ to, ...options })}
			useHref={to => router.buildLocation({ to }).href}
		>
			{import.meta.env.DEV && <DevTools />}

			<div className="h-screen flex flex-col overflow-hidden text-fg-primary">
				<Outlet />
			</div>

			<Toasts />
		</RouterProvider>
	);
}

function DevTools() {
	return (
		<TanStackDevtools
			config={{
				position: 'top-left',
			}}
			plugins={[
				{
					name: 'Tanstack Query',
					render: <ReactQueryDevtoolsPanel />,
				},
				{
					name: 'Tanstack Router',
					render: <TanStackRouterDevtoolsPanel />,
				},
			]}
		/>
	);
}
