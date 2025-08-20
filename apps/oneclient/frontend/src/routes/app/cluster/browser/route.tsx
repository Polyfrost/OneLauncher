import type { Provider } from '@/bindings.gen';
import { Dropdown, Show } from '@onelauncher/common/components';
import { createFileRoute, Outlet } from '@tanstack/react-router';
import { SearchMdIcon } from '@untitled-theme/icons-react';

export interface BrowserRouteSearchParams {
	provider: Provider;
}

export const Route = createFileRoute('/app/cluster/browser')({
	component: RouteComponent,
	validateSearch: (search): BrowserRouteSearchParams => {
		return {
			provider: search.provider as Provider,
		};
	},
});

function RouteComponent() {
	return (
		<Outlet />
	);
}
