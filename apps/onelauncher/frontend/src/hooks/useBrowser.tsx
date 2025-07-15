import type { ClusterModel } from '@/bindings.gen';
import { bindings } from '@/main';
import { PROVIDERS } from '@/utils';
import { useCommand } from '@onelauncher/common';
import { createContext, useContext, useMemo, useState } from 'react';

type Provider = typeof PROVIDERS[number];

export interface BrowserControllerType {
	cluster: ClusterModel | undefined;
	setCluster: (cluster: ClusterModel | undefined) => void;
	provider: Provider;
	setProvider: (provider: Provider) => void;

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

	const context = useMemo<BrowserControllerType>(() => ({
		cluster,
		setCluster,
		provider,
		setProvider,
		search: () => {},
	}), [cluster, provider]);

	return (
		<BrowserContext.Provider value={context}>
			{children}
		</BrowserContext.Provider>
	);
}
