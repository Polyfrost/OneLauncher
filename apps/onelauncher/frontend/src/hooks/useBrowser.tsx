import type { ClusterModel, GameLoader, ManagedPackage, ManagedVersion, PackageCategories, Paginated, Provider, SearchQuery, SearchResult } from '@/bindings.gen';
import type { BindingCommands } from '@/types/global';
import type { UndefinedInitialDataOptions } from '@tanstack/react-query';
import type { Dispatch, SetStateAction } from 'react';
import { bindings } from '@/main';
import { PROVIDERS } from '@/utils';
import { useCommand, useCommandMut } from '@onelauncher/common';
import { useNavigate } from '@tanstack/react-router';
import { createContext, useContext, useEffect, useMemo, useState } from 'react';

export interface BrowserControllerType {
	cluster: ClusterModel | undefined;
	setCluster: Dispatch<SetStateAction<ClusterModel | undefined>>;
	provider: Provider;
	setProvider: Dispatch<SetStateAction<Provider>>;
	query: SearchQuery;
	setQuery: Dispatch<SetStateAction<SearchQuery>>;
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
	const navigate = useNavigate();
	const [query, setQuery] = useState<SearchQuery>({
		filters: {
			categories: null,
			game_versions: null,
			loaders: null,
			package_type: 'mod',
		},
		query: null,
		limit: 18 as unknown as bigint,
		offset: null,
		sort: null,
	});

	useEffect(() => {
		if (query.filters?.categories || query.query)
			navigate({ to: '/app/browser/search' });
	}, [navigate, query]);

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

export function getCategories(categories: PackageCategories) {
	if ('Mod' in categories)
		return categories.Mod;
	if ('ResourcePack' in categories)
		return categories.ResourcePack;
	if ('Shader' in categories)
		return categories.Shader;
	if ('DataPack' in categories)
		return categories.DataPack;
	if ('ModPack' in categories)
		return categories.ModPack;
	return [];
}

export function useBrowserSearch(provider: Provider, query: SearchQuery, options?: Omit<UndefinedInitialDataOptions<Paginated<SearchResult>>, 'queryKey' | 'queryFn'> | undefined) {
	const validFilters = useMemo(() => Object.values(query.filters ?? {}).filter(a => a).length > 0, [query.filters]);
	return useCommand('searchPackages', () => bindings.core.searchPackages(provider, validFilters ? query : { ...query, filters: null }), options);
}

export function usePackageData(provider: Provider, slug: string, options?: Omit<UndefinedInitialDataOptions<ManagedPackage>, 'queryKey' | 'queryFn'> | undefined, key: false | BindingCommands | (string & {}) = `getPackage.${provider}.${slug}`) {
	return useCommand(key, () => bindings.core.getPackage(provider, slug), options);
}

export function usePackageVersions(provider: Provider, slug: string, { mc_version, loader, offset, limit, ...options }: { mc_version?: string | null; loader?: GameLoader | null; offset?: number; limit: number } & Omit<UndefinedInitialDataOptions<Paginated<ManagedVersion>>, 'queryKey' | 'queryFn'>, key: false | BindingCommands | (string & {}) = `getPackageVersions.${provider}.${slug}.${mc_version}.${loader}`) {
	return useCommand(key, () => bindings.core.getPackageVersions(provider, slug, mc_version ?? null, loader ?? null, (offset ?? 0) as unknown as bigint, limit as unknown as bigint), options);
}

export function useDownloadPackage(cluster: ClusterModel, provider: Provider, version: ManagedVersion, skipCompatibility = false) {
	return useCommandMut(() => bindings.core.downloadPackage(provider, version.project_id, version.version_id, cluster.id, skipCompatibility));
}
