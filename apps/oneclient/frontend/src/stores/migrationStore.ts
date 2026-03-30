import type { ClusterModel } from '@/bindings.gen';
import { create } from 'zustand';

export interface MigrationStore {
	isOpen: boolean;
	isDebugPreview: boolean;
	newVersions: Array<string>;
	sourceClusters: Array<ClusterModel>;
	allClusters: Array<ClusterModel>;
}

export interface MigrationStoreActions {
	setAllClusters: (clusters: Array<ClusterModel>) => void;
	setMigrationCandidates: (newVersions: Array<string>, sourceClusters: Array<ClusterModel>) => void;
	setIsOpen: (open: boolean) => void;
	openForDebug: () => void;
}

function getUniqueVersions(clusters: Array<ClusterModel>): Array<string> {
	return [...new Set(clusters.map(cluster => cluster.mc_version))];
}

export const useMigrationStore = create<MigrationStore & MigrationStoreActions>()(set => ({
	isOpen: false,
	isDebugPreview: false,
	newVersions: [],
	sourceClusters: [],
	allClusters: [],

	setAllClusters: clusters => set({ allClusters: clusters }),
	setMigrationCandidates: (newVersions, sourceClusters) => set({ newVersions, sourceClusters, isDebugPreview: false }),
	setIsOpen: open => set(state => ({
		isOpen: open,
		isDebugPreview: open ? state.isDebugPreview : false,
	})),
	openForDebug: () => set((state) => {
		if (state.allClusters.length === 0)
			return state;

		const unplayedClusters = state.allClusters.filter(cluster => cluster.last_played === null);
		const unplayedVersions = getUniqueVersions(unplayedClusters);
		const allVersions = getUniqueVersions(state.allClusters);
		const targetVersions = unplayedVersions.length > 0 ? unplayedVersions : allVersions;
		const sourceClusters = state.allClusters.filter(cluster => !targetVersions.includes(cluster.mc_version));

		return {
			isOpen: true,
			isDebugPreview: true,
			newVersions: targetVersions,
			sourceClusters: sourceClusters.length > 0 ? sourceClusters : state.allClusters,
		};
	}),
}));

export default useMigrationStore;
