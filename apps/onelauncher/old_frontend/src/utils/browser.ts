import type { BrowserListView, PackageType, Providers } from '@onelauncher/client/bindings';

export const BROWSER_VIEWS: BrowserListView[] = ['grid', 'list'] as const;

export interface CategoryItem {
	display: string;
	id: string;
}

export const browserCategories = {
	byPackageType(packageType: PackageType, provider: Providers = 'Modrinth'): CategoryItem[] {
		switch (packageType) {
			case 'mod':
				return this.mod(provider);
		}

		return [];
	},

	mod(provider: Providers = 'Modrinth'): CategoryItem[] {
		return Object.values(modMapping[provider]);
	},
};

const modMapping: Record<Providers, CategoryItem[]> = {
	Curseforge: [
		{ display: 'Adventure', id: '422' },
		{ display: 'API and Library', id: '421' },
		{ display: 'Armor and Tools', id: '434' },
		{ display: 'Biomes', id: '407' },
		{ display: 'Bug Fixes', id: '6821' },
		{ display: 'Cosmetic', id: '424' },
		{ display: 'Dimensions', id: '410' },
		{ display: 'Education', id: '5299' },
		{ display: 'Energy', id: '417' },
		{ display: 'Farming', id: '416' },
		{ display: 'Food', id: '436' },
		{ display: 'Forestry', id: '433' },
		{ display: 'Magic', id: '419' },
		{ display: 'Miscellaneous', id: '425' },
		{ display: 'Mobs', id: '411' },
		{ display: 'Ores and Resources', id: '408' },
		{ display: 'Performance', id: '6814' },
		{ display: 'Player Transport', id: '414' },
		{ display: 'Redstone', id: '4558' },
		{ display: 'Storage', id: '420' },
		{ display: 'Structures', id: '409' },
		{ display: 'Technology', id: '412' },
		{ display: 'Utility & QoL', id: '5191' },
		{ display: 'World Generation', id: '406' },

		// { display: 'Map and Information', id: '423' },
		// { display: 'Server Utility', id: '435' },
		// { display: 'Twitch Integration', id: '4671' },
		// { display: 'Create', id: '6484' },
		// { display: 'Buildcraft', id: '432' },
		// { display: 'Industrial Craft', id: '429' },
		// { display: 'KubeJS', id: '5314' },
		// { display: 'Genetics', id: '418' },
		// { display: 'Blood Magic', id: '4485' },
		// { display: 'Automation', id: '4843' },
		// { display: 'CraftTweaker', id: '4773' },
		// { display: 'Addons', id: '426' },
		// { display: 'Energy, Fluid, and Item Transport', id: '415' },
		// { display: 'Thermal Expansion', id: '427' },
		// { display: 'Thaumcraft', id: '430' },
		// { display: 'Processing', id: '413' },
		// { display: 'Tinker\'s Construct', id: '428' },
		// { display: 'Skyblock', id: '6145' },
		// { display: 'Galacticraft', id: '5232' },
		// { display: 'Applied Energistics 2', id: '4545' },
		// { display: 'Integrated Dynamics', id: '6954' },
		// { display: 'MCreator', id: '4906' },
	],
	Modrinth: [
		{ display: 'Adventure', id: 'adventure' },
		{ display: 'Cursed', id: 'cursed' },
		{ display: 'Decoration', id: 'decoration' },
		{ display: 'Economy', id: 'economy' },
		{ display: 'Equipment', id: 'equipment' },
		{ display: 'Food', id: 'food' },
		{ display: 'Game Mechanics', id: 'game-mechanics' },
		{ display: 'Library', id: 'library' },
		{ display: 'Magic', id: 'magic' },
		{ display: 'Management', id: 'management' },
		{ display: 'Minigame', id: 'minigame' },
		{ display: 'Mobs', id: 'mobs' },
		{ display: 'Optimization', id: 'optimization' },
		{ display: 'Social', id: 'social' },
		{ display: 'Storage', id: 'storage' },
		{ display: 'Technology', id: 'technology' },
		{ display: 'Transportation', id: 'transportation' },
		{ display: 'Utility', id: 'utility' },
		{ display: 'World Generation', id: 'worldgen' },
	],
	SkyClient: [],
};
