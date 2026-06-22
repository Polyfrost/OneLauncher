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
	/**
	 * Patch component of the new (year-based) scheme, e.g. the `2` in `26.1.2`.
	 * `undefined` for two-part new versions (`26.1`) and for all old `1.X.Y` versions
	 * (whose third component is the `minor`, not a patch).
	 */
	patch?: number | undefined;
}

/**
 * The major version at and above which Minecraft uses the new year-based scheme
 * (`YY.N[.P]`, e.g. `26.1` / `26.1.2`). Below it, the legacy `1.X[.Y]` scheme is used.
 *
 * Kept as a single named constant so every formatter/parser agrees on the boundary —
 * an inconsistent boundary is how malformed strings like `1.26` get produced.
 */
export const NEW_SCHEME_MAJOR = 26;

function toNumberOrUndefined(value: string | undefined): number | undefined {
	if (value === undefined)
		return undefined;
	const parsed = Number.parseInt(value, 10);
	return Number.isNaN(parsed) ? undefined : parsed;
}

export function parseMcVersion(version: string | null | undefined): ParsedMcVersion | undefined {
	if (version === undefined || version === null)
		return undefined;

	const parts = version.split('.');

	// New format: YY.N[.P] (e.g. "26.1", "26.1.1", "26.1.2") — no "1." prefix.
	if (parts[0] !== '1') {
		if (parts.length < 2)
			return undefined;
		const major = toNumberOrUndefined(parts[0]);
		if (major === undefined)
			return undefined;
		return {
			major,
			minor: toNumberOrUndefined(parts[1]),
			// Preserve the patch component instead of silently dropping it, so a
			// "26.1.2" cluster is parsed in full rather than collapsing to "26.1".
			patch: toNumberOrUndefined(parts[2]),
		};
	}

	// Old format: 1.X[.Y] (e.g. "1.21.5")
	if (parts.length <= 1)
		return undefined;

	const major = toNumberOrUndefined(parts[1]);
	if (major === undefined)
		return undefined;

	return {
		major,
		minor: toNumberOrUndefined(parts[2]),
		patch: undefined,
	};
}

/**
 * Constructs a Minecraft version string from a major+minor[+patch] tuple.
 * Versions from {@link NEW_SCHEME_MAJOR} onward use the new `YY.N[.P]` format;
 * older versions use `1.X.Y`.
 *
 * The `major >= NEW_SCHEME_MAJOR` check MUST stay in sync with {@link parseMcVersion}'s
 * `parts[0] !== '1'` rule: a `major` of 26 must never be formatted with a `1.` prefix
 * (which would yield bogus strings like `1.26`).
 */
export function formatMcVersion(major: number, minor: number, patch?: number): string {
	if (major >= NEW_SCHEME_MAJOR)
		return patch === undefined ? `${major}.${minor}` : `${major}.${minor}.${patch}`;
	return `1.${major}.${minor}`;
}
