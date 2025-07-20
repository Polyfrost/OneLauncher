import { PackageGrid } from '@/components/content/PackageItem';
import { useBrowserContext, useBrowserSearch } from '@/hooks/useBrowser';
import { Show } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { useEffect, useMemo, useState } from 'react';
import { BrowserLayout } from './route';
import type { Paginated, SearchResult } from '@/bindings.gen';
import type { PaginationOptions } from '@/hooks/usePagination';
import usePagination from '@/hooks/usePagination';

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
	}, [context.provider, context.query, context.cluster]);

	return (
		<>
			{
			search.data
				? <Results results={search.data}/>
				: <h1>Loading</h1>
			}
		</>
	);
}

function Results({results}:{results:Paginated<SearchResult>}){
	const context = useBrowserContext();
	const [oldTotal, setOldTotal] = useState(results.total)
	const pagination = usePagination({
		itemsCount: results.total as unknown as number,
		itemsPerPage: context.query.limit as unknown as number
	})
	useEffect(()=>{
		context.setQuery({...context.query, offset: pagination.offset as unknown as bigint})
	},[pagination.page])
	useEffect(()=>{
		if(oldTotal === results.total) return;
		pagination.reset()
		console.log("pagination reset", results.total)
		setOldTotal(results.total)
	},[results.total])
	return (
		<div>
			<div className='w-full flex justify-end my-2'>
				<pagination.Navigation/>
			</div>
			{results.items.length == 0
			? <h2 className='text-center text-lg font-light italic opacity-60'>No results</h2>
			: <PackageGrid items={results.items} provider={context.provider}/>
		}

		</div>
	)
}
