import { create } from 'zustand';

export interface AppShellStore {
	activeClusterId: number;
}

export interface AppShellStoreActions {
	setActiveClusterId: (cluster: AppShellStore['activeClusterId']) => void;
}

export const useAppShellStore = create<AppShellStore & AppShellStoreActions>()(set => ({
	activeClusterId: 0,

	setActiveClusterId: cluster => set({ activeClusterId: cluster }),
}));

export default useAppShellStore;
