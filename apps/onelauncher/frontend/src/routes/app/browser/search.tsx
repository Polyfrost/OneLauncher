import type { Paginated, SearchResult } from '@/bindings.gen';
import { PackageGrid } from '@/components/content/PackageItem';
import { useBrowserContext, useBrowserSearch } from '@/hooks/useBrowser';
import usePagination from '@/hooks/usePagination';
import { createFileRoute } from '@tanstack/react-router';
import { useEffect, useRef, useState } from 'react';
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
	}, [context.provider, context.query, context.cluster, search]);

	return (
		<>
			{
				search.data
					? <Results results={search.data} />
					: <h1>Loading</h1>
			}
		</>
	);
}

function Results({ results }: { results: Paginated<SearchResult> }) {
	const context = useBrowserContext();
	const [oldTotal, setOldTotal] = useState(results.total);

	const pagination = usePagination({
		itemsCount: results.total as unknown as number,
		itemsPerPage: context.query.limit as unknown as number,
	});

	const contextRef = useRef(context);

	useEffect(() => {
		contextRef.current.setQuery(query => ({ ...query, offset: pagination.offset as unknown as bigint }));
	}, [pagination.offset, pagination.page]);

	useEffect(() => {
		if (oldTotal === results.total)
			return;
		pagination.reset();
		setOldTotal(results.total);
	}, [oldTotal, pagination, results.total]);

	return (
		<div>
			<div className="w-full flex justify-end my-2">
				<pagination.Navigation />
			</div>
			{results.items.length === 0
				? <h4 className="text-center text-lg font-light opacity-60">No results</h4>
				: <PackageGrid items={results.items} provider={context.provider} />}

		</div>
	);
}
