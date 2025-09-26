import type { Provider } from '@/bindings.gen';
import { SheetPage } from '@/components';
import { createFileRoute, Outlet } from '@tanstack/react-router';

export interface BrowserRouteSearchParams {
	provider: Provider;
}

export const Route = createFileRoute('/app/cluster/browser')({
	component: RouteComponent,
	validateSearch: (search): BrowserRouteSearchParams => {
		return {
			provider: (search.provider || 'Modrinth') as Provider,
		};
	},
});

function RouteComponent() {
	return (
		<SheetPage.Content>
			<Outlet />
		</SheetPage.Content>
	);
}
