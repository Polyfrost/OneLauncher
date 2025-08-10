import type { QueryClient } from '@tanstack/react-query';
import type { NavigateOptions, ToOptions } from '@tanstack/react-router';
import { createRootRouteWithContext, Outlet, useRouter } from '@tanstack/react-router';
import { TanStackRouterDevtools } from '@tanstack/react-router-devtools';
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
		// <AnimatedOutletProvider>
		<RouterProvider
			navigate={(to, options) => router.navigate({ to, ...options })}
			useHref={to => router.buildLocation({ to }).href}
		>
			<div className="h-screen flex flex-col overflow-hidden text-fg-primary">
				<Outlet />

				<TanStackRouterDevtools />
			</div>
		</RouterProvider>
		// </AnimatedOutletProvider>
	);
}
