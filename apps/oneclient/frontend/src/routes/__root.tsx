import type { QueryClient } from '@tanstack/react-query';
import type { NavigateOptions, ToOptions } from '@tanstack/react-router';
import { Toasts } from '@/components';
import { useSettings } from '@/hooks/useSettings';
import { bindings } from '@/main';
import { checkForUpdate, installUpdate, listenForUpdateEvents } from '@/utils/updater';
import { useCommand } from '@onelauncher/common';
import { TanStackDevtools } from '@tanstack/react-devtools';
import { ReactQueryDevtoolsPanel } from '@tanstack/react-query-devtools';
import { createRootRouteWithContext, Outlet, useLocation, useRouter } from '@tanstack/react-router';
import { TanStackRouterDevtoolsPanel } from '@tanstack/react-router-devtools';
import { useEffect } from 'react';
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

type URLPath = Exclude<ToOptions['to'], undefined>;
const ResolvedPathNames: Record<URLPath, string> = {
	'.': 'UNKNOWN',
	'..': 'UNKNOWN',
	'/': 'Viewing Home',
	'/app': 'Viewing Homepage',
	'/app/account': 'Viewing Account',
	'/app/account/skins': 'Viewing Skin Manager',
	'/app/cluster': 'Viewing Clusters',
	'/app/cluster/browser': 'Viewing {clusterName}\'s mods',
	'/app/cluster/browser/package': 'Browsing {packageName}',
	'/app/cluster/logs': 'Viewing {clusterName}\'s logs',
	'/app/cluster/mods': 'Viewing {clusterName}\'s mods',
	'/app/cluster/process': 'Viewing {clusterName}',
	'/app/cluster/settings': 'Viewing {clusterName}\'s settings',
	'/app/settings': 'Viewing Settings',
	'/app/settings/appearance': 'Viewing Settings',
	'/app/settings/developer': 'Viewing Settings',
	'/app/settings/minecraft': 'Viewing Settings',
	'/app/accounts': 'Viewing Accounts',
	'/app/clusters': 'Viewing Clusters',
	'/onboarding': 'Preparing OneClient',
	'/onboarding/account': 'Preparing OneClient',
	'/onboarding/finished': 'Preparing OneClient',
	'/onboarding/language': 'Preparing OneClient',
	'/onboarding/preferences/version': 'Preparing OneClient',
	'/onboarding/preferences/versionCategory': 'Preparing OneClient',
	'/onboarding/preferences': 'Preparing OneClient',
};

// Credit - https://github.com/DuckySoLucky/hypixel-discord-chat-bridge/blob/d3ea84a26ebf094c8191d50b4954549e2dd4dc7f/src/contracts/helperFunctions.js#L216-L225
function ReplaceVariables(template: string, variables: Record<string, any>) {
	return template.replace(/\{(\w+)\}/g, (match: any, name: string | number) => variables[name] ?? match);
}

function useDiscordRPC() {
	const location = useLocation();
	const clusterId = location.search.clusterId ?? 0;
	const provider = location.search.provider ?? null;
	const packageId = location.search.packageId ?? null;
	const { data: cluster } = useCommand(['getClusterById', clusterId], () => bindings.core.getClusterById(clusterId));
	const { data: managedPackage } = useCommand(['getPackage', provider, packageId], () => bindings.core.getPackage(provider!, packageId!), { enabled: provider != null && packageId != null });
	useEffect(() => {
		bindings.core.setDiscordRPCMessage(ReplaceVariables(ResolvedPathNames[location.pathname as URLPath], { clusterName: cluster?.name ?? 'UNKNOWN', packageName: managedPackage?.name ?? 'UNKNOWN' }));
	}, [location.pathname, location.search.clusterId, cluster?.name, managedPackage?.name]);
}

function useAutoUpdate() {
	useEffect(() => {
		const unlistenPromise = listenForUpdateEvents(async (event) => {
			console.log('Updater event:', event);
			if (event.status === 'updateAvailable') {
				console.log('Update available, installing...');
				try {
					await installUpdate();
				}
				catch (e) {
					console.error('Failed to install update:', e);
				}
			}
		});

		checkForUpdate().then((update) => {
			if (update)
				console.log('Update found on initial check:', update.version);
		}).catch(e => console.error('Failed to check for update:', e));

		return () => {
			unlistenPromise.then(unlisten => unlisten());
		};
	}, []);
}

function RootRoute() {
	const router = useRouter();
	const { setting } = useSettings();
	useDiscordRPC();
	useAutoUpdate();
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
