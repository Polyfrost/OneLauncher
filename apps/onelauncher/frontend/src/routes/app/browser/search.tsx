import { PackageGrid } from '@/components/content/PackageItem';
import { useBrowserContext, useBrowserSearch } from '@/hooks/useBrowser';
import { Show } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { useEffect } from 'react';
import { BrowserLayout } from './route';

export const Route = createFileRoute('/app/browser/search')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<BrowserLayout>
			<div className="flex flex-col gap-8">
				<Search />
			</div>
		</BrowserLayout>
	);
}

function Search() {
	const context = useBrowserContext();
	const search = useBrowserSearch(context.provider, context.query, {
	});
	useEffect(() => {
		search.refetch();
	}, [context.provider, context.query]);

	return (
		<Show when={search.isSuccess}>
			<PackageGrid items={search.data!.items} provider={context.provider} />
		</Show>
	);
}
