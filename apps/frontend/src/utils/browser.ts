import type { BrowserListView, PackageType, Providers } from '@onelauncher/client/bindings';

export const BROWSER_VIEWS: BrowserListView[] = ['grid', 'list'] as const;

export interface CategoryItem {
	display: string;
	id: string;
}

type ModCategories = [
	'adventure',
	'library',
	'equipment',
	'decoration',
	'food',
	'magic',
	'performance',
	'storage',
	'technology',
	'utility',
	'worldgen',
];

const modMapping: Record<Providers, { [key in ModCategories[number]]: CategoryItem }> = {
	Modrinth: {
		adventure: {
			display: 'Adventure',
			id: 'adventure',
		},
		library: {
			display: 'Library',
			id: 'library',
		},
		decoration: {
			display: 'Decoration',
			id: 'decoration',
		},
		equipment: {
			display: 'Equipment',
			id: 'equipment',
		},
		food: {
			display: 'Food',
			id: 'food',
		},
		magic: {
			display: 'Magic',
			id: 'magic',
		},
		performance: {
			display: 'Performance',
			id: 'optimization',
		},
		storage: {
			display: 'Storage',
			id: 'storage',
		},
		technology: {
			display: 'Technology',
			id: 'technology',
		},
		utility: {
			display: 'Utility',
			id: 'utility',
		},
		worldgen: {
			display: 'World Generation',
			id: 'worldgen',
		},
	},
};

export const browserCategories = {
	byPackageType(packageType: PackageType): CategoryItem[] {
		switch (packageType) {
			case 'mod':
				return this.mod();
		}

		return [];
	},

	mod(provider: Providers = 'Modrinth'): CategoryItem[] {
		return Object.values(modMapping[provider]);
	},
};
