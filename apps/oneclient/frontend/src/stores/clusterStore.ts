import type { GameLoader } from '@/bindings.gen';
import type { ParsedMcVersion } from '@/utils/versionMap';
import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type MajorVersion = number;
export type MinorVersion = number;

export interface ClusterStore {
	modLoaders: Record<`${MajorVersion}.${MinorVersion}`, GameLoader>;
	minorVersions: Array<ParsedMcVersion>;
}

export interface ClusterStoreActions {
	setModLoader: (version: `${MajorVersion}.${MinorVersion}`, loader: GameLoader) => void;
	setMinorVersion: (major: number, minor: number) => void;
}

export const useClusterStore = create<ClusterStore & ClusterStoreActions>()(persist(
	set => ({
		modLoaders: {},
		minorVersions: [],

		setModLoader: (version, loader) => set(state => ({
			modLoaders: {
				...state.modLoaders,
				[version]: loader,
			},
		})),
		setMinorVersion: (major, minor) => set((state) => {
			const existing = state.minorVersions.find(v => v.major === major);
			if (existing)
				existing.minor = minor;
			else
				state.minorVersions.push({ major, minor });

			return { minorVersions: [...state.minorVersions] };
		}),
	}),
	{
		name: 'oneclient:cluster_list',
	},
));

export default useClusterStore;
