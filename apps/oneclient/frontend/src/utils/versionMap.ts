import type { GameBackgroundName } from '@/components';

export interface VersionInfo {
	backgroundName: GameBackgroundName;
	prettyName: string;
	shortDescription: string;
	longDescription: string;
	tags: Array<string>;
}

const VERSION_MAP: Record<number, VersionInfo> = {
	[-1]: {
		backgroundName: 'HypixelSkyblockHub',
		shortDescription: 'Unknown Version',
		longDescription: 'This version is not recognized or does not have specific information available.',
		prettyName: '0.0',
		tags: [],
	},
	8: {
		backgroundName: 'HypixelSkyblockHub',
		prettyName: '1.8',
		shortDescription: 'The Bountiful Update',
		longDescription: 'The Bountiful Update is a major update that introduces new features, items, and gameplay mechanics to enhance the Minecraft experience. It focuses on expanding the game world and providing players with new challenges and adventures.',
		tags: ['Legacy', 'Bountiful Update'],
	},
	21: {
		backgroundName: 'CavesAndCliffs',
		prettyName: '1.21',
		shortDescription: 'The Tricky Trials Update',
		longDescription: `Minecraft's 1.21 update, known as "Tricky Trials," primarily focuses on combat adventures and tinkering, introducing trial chambers, new copper block variants, a new crafting tool, and a new weapon. It also features new hostile mobs, paintings, and gameplay enhancements.`,
		tags: ['Tricky Trials'],
	},
} as const;

export function getVersionInfo(version: number | string | null | undefined): VersionInfo | undefined {
	if (version === undefined || version === null)
		return undefined;

	let majorVersion: number | undefined;

	if (typeof version === 'string')
		majorVersion = parseMcVersion(version)?.major;
	else if (typeof version === 'number')
		majorVersion = version;

	if (majorVersion)
		return VERSION_MAP[majorVersion];
	else
		return undefined;
}

export function getVersionInfoOrDefault(version: number | string | null | undefined): VersionInfo {
	const info = getVersionInfo(version);
	return info ?? VERSION_MAP[-1];
}

export interface ParsedMcVersion {
	major: number;
	minor: number | undefined;
}

export function parseMcVersion(version: string | null | undefined): ParsedMcVersion | undefined {
	if (version === undefined || version === null)
		return undefined;

	const parts = version.split('.');
	if (parts.length <= 1) // we need a.b.c where c is optional
		return undefined;

	const major = Number.parseInt(parts[1], 10);
	const minor = parts.length > 1 ? Number.parseInt(parts[2], 10) : undefined;

	return {
		major,
		minor: Number.isNaN(minor) ? undefined : minor,
	};
}
