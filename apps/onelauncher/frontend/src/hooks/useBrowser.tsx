import type { ClusterModel, Paginated, Provider, SearchQuery, SearchResult } from '@/bindings.gen';
import type { UndefinedInitialDataOptions } from '@tanstack/react-query';
import { bindings } from '@/main';
import { PROVIDERS } from '@/utils';
import { useCommand } from '@onelauncher/common';
import { createContext, useContext, useEffect, useMemo, useState } from 'react';

export interface BrowserControllerType {
	cluster: ClusterModel | undefined;
	setCluster: (cluster: ClusterModel | undefined) => void;
	provider: Provider;
	setProvider: (provider: Provider) => void;
	query: SearchQuery;
	setQuery: (query: SearchQuery) => void;
	search: () => void;
}

const BrowserContext = createContext<BrowserControllerType | null>(null);

export function useBrowserContext() {
	const context = useContext(BrowserContext);
	if (!context)
		throw new Error('useBrowserContext must be used within a BrowserProvider');
	return context;
}

export function BrowserProvider(props: any) {
	const { children } = props;
	const [cluster, setCluster] = useState<ClusterModel | undefined>(undefined);
	const [provider, setProvider] = useState<Provider>(PROVIDERS[0]);
	const [query, setQuery] = useState<SearchQuery>({
		filters: null,
		query: null,
		limit: 18 as unknown as bigint,
		offset: null,
		sort: null,
	});

	const context = useMemo<BrowserControllerType>(() => ({
		cluster,
		setCluster,
		provider,
		setProvider,
		query,
		setQuery,
		search: () => {},
	}), [cluster, provider, query]);

	return (
		<BrowserContext.Provider value={context}>
			{children}
		</BrowserContext.Provider>
	);
}

export function useBrowserSearch(provider: Provider, query: SearchQuery, options?: Omit<UndefinedInitialDataOptions<Paginated<SearchResult>>, 'queryKey' | 'queryFn'> | undefined) {
	const validFilters = useMemo(() => Object.values(query.filters ?? {}).filter(a => a).length > 0, [query.filters]);
	return useCommand('searchPackages', () => bindings.core.searchPackages(provider, validFilters ? query : { ...query, filters: null }), options);
}
