import type { ParsedLocation } from '@tanstack/react-router';
import { create } from 'zustand';

export interface AppShellStore {
	background: 'gradientOverlay' | 'none';
	prevLocation: ParsedLocation | null;
}

export interface AppShellStoreActions {
	setBackground: (background: AppShellStore['background']) => void;
	setPrevLocation: (location: ParsedLocation | null) => void;
}

export const useAppShellStore = create<AppShellStore & AppShellStoreActions>()(set => ({
	background: 'gradientOverlay',
	prevLocation: null,

	setBackground: background => set({ background }),
	setPrevLocation: location => set({ prevLocation: location }),
}));

export default useAppShellStore;
