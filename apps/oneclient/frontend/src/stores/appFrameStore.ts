import type { ClusterModel } from '@/bindings.gen';
import type { FileRoutesByFullPath } from '@/routeTree.gen';
import { create } from 'zustand';

interface AppFrameStore {
	previousPage: FileRoutesByFullPath | null;
	activeCluster: ClusterModel | null;
}

const useAppFrameStore = create<AppFrameStore>()(set => ({
	activeCluster: null,
	previousPage: null,

	setPreviousPage: (page: FileRoutesByFullPath) => set({ previousPage: page }),
	setActiveCluster: (cluster: ClusterModel | null) => set({ activeCluster: cluster }),
}));

export default useAppFrameStore;
