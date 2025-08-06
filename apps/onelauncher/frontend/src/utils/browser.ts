import type { PackageCategories, PackageModCategory, PackageType, Provider } from '@/bindings.gen';
import type { Key } from '.';

export const BROWSER_VIEWS: Array<string> = ['grid', 'list'] as const;

const ModCategories = [
	'Adventure',
	'Library',
	'Equipment',
	'Patches',
	'Cosmetic',
	'Food',
	'Magic',
	'Information',
	'Misc',
	'Performance',
	'Redstone',
	'ServerUtil',
	'Storage',
	'Technology',
	'Farming',
	'Automation',
	'Transport',
	'Utility',
	'QoL',
	'WorldGen',
	'Mobs',
	'Economy',
	'Social',
] as const;

const ModpackCategories = [
	'Technology',
	'Quests',
	'Optimization',
	'Multiplayer',
	'Magic',
	'LightWeight',
	'Combat',
	'Challenging',
	'Adventure',
] as const;

const ResourcePackCategories = [
	'X8',
	'X16',
	'X32',
	'X48',
	'X64',
	'X128',
	'X256',
	'X512',
	'VanillaLike',
	'Utility',
	'Tweaks',
	'Themed',
	'Simplistic',
	'Realistic',
	'Modded',
	'Decoration',
	'Cursed',
	'Combat',
	'Audio',
	'Blocks',
	'CoreShaders',
	'Gui',
	'Fonts',
	'Equipment',
	'Environment',
	'Entities',
	'Items',
	'Locale',
	'Models',
] as const;

const ShaderCategories = [
	'VanillaLike',
	'SemiRealistic',
	'Realistic',
	'Fantasy',
	'Cursed',
	'Cartoon',
	'Bloom',
	'Atmosphere',
	'Reflections',
	'Shadows',
	'PBR',
	'PathTracing',
	'Foliage',
	'ColoredLightning',
	'Potato',
	'Low',
	'Medium',
	'High',
	'Ultra',
] as const;

export const browserCategories = {
	Mod: ModCategories,
	ResourcePack: ResourcePackCategories,
	ModPack: ModpackCategories,
	Shader: ShaderCategories,
	DataPack: ModCategories,
} as const satisfies Record<Key<PackageCategories>, ReadonlyArray<string>>;

type CategoryIdMap = {
	[K in keyof typeof browserCategories as Lowercase<K>]: K
};

export const categoryNameFromId: CategoryIdMap = {
	mod: 'Mod',
	resourcepack: 'ResourcePack',
	modpack: 'ModPack',
	shader: 'Shader',
	datapack: 'DataPack',
};
