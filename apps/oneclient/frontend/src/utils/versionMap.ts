import type { GameBackgroundName } from '@/components';

export interface VersionInfo {
	backgroundName: GameBackgroundName;
	description: string;
}

const VERSION_MAP: Record<number, VersionInfo> = {
	[-1]: {
		backgroundName: 'HypixelSkyblockHub',
		description: 'Unknown Version',
	},
	8: {
		backgroundName: 'HypixelSkyblockHub',
		description: 'The Bountiful Update',
	},
} as const;

export function getVersionInfo(version: number | string | null | undefined): VersionInfo | undefined {
	if (version === undefined || version === null)
		return undefined;

	let majorVersion;

	if (typeof version === 'string') {
		const parts = version.split('.');
		if (parts.length >= 2)
			majorVersion = Number.parseInt(parts[1], 10);
	}

	if (majorVersion)
		return VERSION_MAP[majorVersion];
	else
		return undefined;
}

export function getVersionInfoOrDefault(version: number | string): VersionInfo {
	const info = getVersionInfo(version);
	return info ?? VERSION_MAP[-1];
}
