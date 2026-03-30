import type { OnlineCluster, OnlineClusterEntry, OnlineClusterManifest } from '@/bindings.gen';
import type { GameBackgroundName } from '@/components';

export interface VersionInfo {
	backgroundName: GameBackgroundName;
	prettyName: string;
	shortDescription: string;
	longDescription: string;
	tags: Array<string>;
}

const DEFAULT_VERSION_INFO: VersionInfo = {
	backgroundName: 'MinecraftBuilding',
	prettyName: '?',
	shortDescription: 'Unknown Version',
	longDescription: 'This version is not recognized or does not have specific information available.',
	tags: [],
};

export function getVersionInfoFromCluster(cluster: OnlineCluster): VersionInfo {
	const { major_version } = cluster;
	return {
		backgroundName: 'MinecraftBuilding',
		prettyName: major_version >= 26 ? `${major_version}` : `1.${major_version}`,
		shortDescription: cluster.name,
		longDescription: cluster.long_description ?? '',
		tags: cluster.tags,
	};
}

export function getVersionInfo(
	version: number | string | null | undefined,
	manifest: OnlineClusterManifest,
): VersionInfo | undefined {
	const cluster = getOnlineClusterForVersion(version, manifest);
	if (!cluster)
		return undefined;
	return getVersionInfoFromCluster(cluster);
}

export function getVersionInfoOrDefault(
	version: number | string | null | undefined,
	manifest: OnlineClusterManifest,
): VersionInfo {
	return getVersionInfo(version, manifest) ?? DEFAULT_VERSION_INFO;
}

export function getOnlineClusterForVersion(
	version: number | string | null | undefined,
	versions: OnlineClusterManifest,
): OnlineCluster | undefined {
	let major: number | undefined;
	if (typeof version === 'string')
		major = parseMcVersion(version)?.major;
	else if (typeof version === 'number')
		major = version;
	if (major === undefined)
		return undefined;
	return versions.clusters.find(c => c.major_version === major);
}

export function getOnlineEntryForVersion(
	version: string | null | undefined,
	versions: OnlineClusterManifest,
): OnlineClusterEntry | undefined {
	const parsed = parseMcVersion(version);
	if (!parsed || parsed.minor === undefined)
		return undefined;
	const cluster = versions.clusters.find(c => c.major_version === parsed.major);
	if (!cluster)
		return undefined;
	return cluster.entries.find(e => e.minor_version === parsed.minor);
}

export interface ParsedMcVersion {
	major: number;
	minor: number | undefined;
}

export function parseMcVersion(version: string | null | undefined): ParsedMcVersion | undefined {
	if (version === undefined || version === null)
		return undefined;

	const parts = version.split('.');

	// New format: YY.N[.P] (e.g. "26.1" or "26.1.1") — no "1." prefix
	if (parts[0] !== '1') {
		if (parts.length < 2)
			return undefined;
		const major = Number.parseInt(parts[0], 10);
		const minor = Number.parseInt(parts[1], 10);
		return {
			major,
			minor: Number.isNaN(minor) ? undefined : minor,
		};
	}

	// Old format: 1.X[.Y] (e.g. "1.21.5")
	if (parts.length <= 1)
		return undefined;

	const major = Number.parseInt(parts[1], 10);
	const minor = parts.length > 2 ? Number.parseInt(parts[2], 10) : undefined;

	return {
		major,
		minor: Number.isNaN(minor) ? undefined : minor,
	};
}

/**
 * Constructs a Minecraft version string from a major+minor pair.
 * Versions from 2026 onward use the new YY.N format; older versions use 1.X.Y.
 */
export function formatMcVersion(major: number, minor: number): string {
	if (major >= 26)
		return `${major}.${minor}`;
	return `1.${major}.${minor}`;
}
