import type { ClusterModel } from '@/bindings.gen';
import { createContext } from 'react';

export interface BrowserControllerType {
	cluster: ClusterModel | undefined;
	setCluster: (cluster: ClusterModel | undefined) => void;

	search: () => void;
}

const browserContext = createContext<BrowserControllerType | null>(null);

export function BrowserProvider(props: any) {
	const { children } = props;

	return (
		<browserContext.Provider value={browserContext}>
			{children}
		</browserContext.Provider>
	);
}
