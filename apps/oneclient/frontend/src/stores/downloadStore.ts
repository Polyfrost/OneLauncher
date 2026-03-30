import type { ModpackArchive } from '@/bindings.gen';
import type { ModWithBundle } from '@/components';
import { create } from 'zustand';

export interface DownloadStore {
	modsPerCluster: Record<string, Array<ModWithBundle>>;
	bundlesPerCluster: Record<number, Array<ModpackArchive>>;
}

export interface DownloadStoreActions {
	setDownloadData: (modsPerCluster: Record<string, Array<ModWithBundle>>, bundlesPerCluster: Record<number, Array<ModpackArchive>>) => void;
	clear: () => void;
}

export const useDownloadStore = create<DownloadStore & DownloadStoreActions>()(set => ({
	modsPerCluster: {},
	bundlesPerCluster: {},

	setDownloadData: (modsPerCluster, bundlesPerCluster) => set({ modsPerCluster, bundlesPerCluster }),
	clear: () => set({ modsPerCluster: {}, bundlesPerCluster: {} }),
}));

export default useDownloadStore;
