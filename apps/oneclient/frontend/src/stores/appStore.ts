import type { ClusterModel } from '@/bindings.gen';
import type { FileRoutesByFullPath } from '@/routeTree.gen';
import { create } from 'zustand';

interface AppStore {
	activeCluster: ClusterModel | null;
	previousPage: FileRoutesByFullPath | null;
}

interface AppStoreActions {
	setInitialData: (data: AppStore) => void;
	setActiveCluster: (cluster: ClusterModel | null) => void;
	setPreviousPage: (page: FileRoutesByFullPath) => void;
}

const useAppStore = create<AppStore & AppStoreActions>()(set => ({
	activeCluster: null,
	previousPage: null,

	setInitialData: (data: AppStore) => set({
		activeCluster: data.activeCluster,
		previousPage: data.previousPage,
	}),
	setPreviousPage: (page: FileRoutesByFullPath) => set({ previousPage: page }),
	setActiveCluster: (cluster: ClusterModel | null) => set({ activeCluster: cluster }),
}));

export default useAppStore;
