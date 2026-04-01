import { bindings } from '@/main';
import useMigrationStore from '@/stores/migrationStore';
import { useCommandSuspense } from '@onelauncher/common';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

const DEFAULT_SEEN_VERSIONS = ['1.21.1', '1.21.10', '1.21.11'] as const;

function areStringArraysEqual(left: Array<string>, right: Array<string>): boolean {
	return left.length === right.length && left.every((value, index) => value === right[index]);
}

function areClustersEqual(
	left: Array<{ id: number; mc_version: string; mc_loader: string; last_played: string | null }>,
	right: Array<{ id: number; mc_version: string; mc_loader: string; last_played: string | null }>,
): boolean {
	return left.length === right.length
		&& left.every((cluster, index) => {
			const other = right[index];
			return cluster.id === other.id
				&& cluster.mc_version === other.mc_version
				&& cluster.mc_loader === other.mc_loader
				&& cluster.last_played === other.last_played;
		});
}

export function useVersionMigration() {
	const queryClient = useQueryClient();
	const { data: grouped } = useCommandSuspense(
		['getClustersGroupedByMajor'],
		bindings.oneclient.getClustersGroupedByMajor,
	);
	const { data: settingsData } = useCommandSuspense(['readSettings'], bindings.core.readSettings);
	const isOpen = useMigrationStore(state => state.isOpen);
	const isDebugPreview = useMigrationStore(state => state.isDebugPreview);
	const newVersions = useMigrationStore(state => state.newVersions);
	const sourceClusters = useMigrationStore(state => state.sourceClusters);
	const allClusters = useMigrationStore(state => state.allClusters);
	const setAllClusters = useMigrationStore(state => state.setAllClusters);
	const setMigrationCandidates = useMigrationStore(state => state.setMigrationCandidates);
	const setIsOpen = useMigrationStore(state => state.setIsOpen);
	const openForDebug = useMigrationStore(state => state.openForDebug);

	useEffect(() => {
		const settings = settingsData;
		const storedSeenVersions = Array.isArray(settings.seen_versions) ? settings.seen_versions : [];
		const seenVersions = [
			...new Set([
				...DEFAULT_SEEN_VERSIONS,
				...storedSeenVersions,
			]),
		];
		const nextAllClusters = Object.values(grouped)
			.flat()
			.filter((cluster): cluster is NonNullable<typeof cluster> => Boolean(cluster));
		const currentVersions = [...new Set(nextAllClusters.map(c => c.mc_version))];

		const unseen = currentVersions.filter(v => !seenVersions.includes(v));
		const unseenUnplayed = unseen.filter(v =>
			nextAllClusters.some(c => c.mc_version === v && c.last_played === null));

		// Clusters not belonging to any new version — candidates to copy from
		const sources = nextAllClusters.filter(c => !unseenUnplayed.includes(c.mc_version));
		const migrationState = useMigrationStore.getState();

		if (!areClustersEqual(migrationState.allClusters, nextAllClusters))
			setAllClusters(nextAllClusters);

		const migrationCandidatesChanged
			= !areStringArraysEqual(migrationState.newVersions, unseenUnplayed)
				|| !areClustersEqual(migrationState.sourceClusters, sources);

		// Keep active modal data stable while it is open.
		if (migrationCandidatesChanged && !migrationState.isOpen)
			setMigrationCandidates(unseenUnplayed, sources);

		if (unseenUnplayed.length > 0 && sources.length > 0 && !migrationState.isOpen && settings.seen_onboarding)
			setIsOpen(true);

		const nextSeenVersions = [...new Set([...currentVersions, ...DEFAULT_SEEN_VERSIONS])].sort();
		const normalizedStoredSeenVersions = [...storedSeenVersions].sort();
		const hasSeenVersionsChanged
			= !areStringArraysEqual(nextSeenVersions, normalizedStoredSeenVersions);

		if (hasSeenVersionsChanged) {
			const nextSettings = {
				...settings,
				seen_versions: nextSeenVersions,
			} as typeof settingsData;

			queryClient.setQueryData(['readSettings'], nextSettings);
			void bindings.core.writeSettings(nextSettings);
		}
	}, [grouped, queryClient, setAllClusters, setIsOpen, setMigrationCandidates, settingsData]);

	return { isOpen, isDebugPreview, setIsOpen, newVersions, sourceClusters, allClusters, openForDebug };
}
