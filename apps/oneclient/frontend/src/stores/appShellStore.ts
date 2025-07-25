import { create } from 'zustand';

export interface AppShellStore {
	background: 'gradientOverlay' | 'none';
}

export interface AppShellStoreActions {
	setBackground: (background: AppShellStore['background']) => void;
}

export const useAppShellStore = create<AppShellStore & AppShellStoreActions>()(set => ({
	background: 'gradientOverlay',

	setBackground: background => set({ background }),
}));

export default useAppShellStore;
