import type { ModpackArchive } from '@/bindings.gen';
import type { ModWithBundle } from '@/components';
import { create } from 'zustand';

export interface DownloadStore {
	modsPerCluster: Record<string, Array<ModWithBundle>>;
	bundlesPerCluster: Record<number, Array<ModpackArchive>>;
	isFinishingOnboarding: boolean;
}

export interface DownloadStoreActions {
	setDownloadData: (modsPerCluster: Record<string, Array<ModWithBundle>>, bundlesPerCluster: Record<number, Array<ModpackArchive>>) => void;
	markOnboardingFinishing: () => void;
	resetOnboardingFinishing: () => void;
	clear: () => void;
}

export const useDownloadStore = create<DownloadStore & DownloadStoreActions>()(set => ({
	modsPerCluster: {},
	bundlesPerCluster: {},
	isFinishingOnboarding: false,

	setDownloadData: (modsPerCluster, bundlesPerCluster) => set({
		modsPerCluster,
		bundlesPerCluster,
		isFinishingOnboarding: false,
	}),
	markOnboardingFinishing: () => set({ isFinishingOnboarding: true }),
	resetOnboardingFinishing: () => set({ isFinishingOnboarding: false }),
	clear: () => set(state => ({
		modsPerCluster: {},
		bundlesPerCluster: {},
		isFinishingOnboarding: state.isFinishingOnboarding,
	})),
}));

export default useDownloadStore;
