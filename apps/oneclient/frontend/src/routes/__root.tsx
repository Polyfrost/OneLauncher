import type { QueryClient } from '@tanstack/react-query';
import type { NavigateOptions, ToOptions } from '@tanstack/react-router';
import { Toasts } from '@/components';
import { MigrationModal } from '@/components/overlay/MigrationModal';
import { useDebugKeybind } from '@/hooks/useDebugInfo';
import { useDiscordRPC } from '@/hooks/useDiscordRPC';
import { useSettings } from '@/hooks/useSettings';
import { useAutoUpdater } from '@/hooks/useUpdater';
import { useVersionMigration } from '@/hooks/useVersionMigration';
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
	const { setting } = useSettings();
	useDiscordRPC();
	useAutoUpdater();
	useDebugKeybind();

	const migration = useVersionMigration();

	return (
		<RouterProvider
			navigate={(to, options) => router.navigate({ to, ...options })}
			useHref={to => router.buildLocation({ to }).href}
		>
			{setting('show_tanstack_dev_tools') && <DevTools />}

			<div className="h-screen flex flex-col overflow-hidden text-fg-primary">
				<Outlet />
			</div>

			<Toasts />

			{setting('seen_onboarding') && (
				<MigrationModal
					allClusters={migration.allClusters}
					isDebugPreview={migration.isDebugPreview}
					isOpen={migration.isOpen}
					newVersions={migration.newVersions}
					onOpenChange={migration.setIsOpen}
					sourceClusters={migration.sourceClusters}
				/>
			)}
		</RouterProvider>
	);
}

function DevTools() {
	return (
		<TanStackDevtools
			config={{
				position: 'bottom-left',
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
