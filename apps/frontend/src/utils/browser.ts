import type { BrowserListView, PackageType } from '@onelauncher/client/bindings';

export const BROWSER_VIEWS: BrowserListView[] = ['grid', 'list', 'preview'] as const;

export interface BrowserSidebarCategory {
	name: string;
	sub: string[];
};

export const browserCategories = {
	byPackageType(packageType: PackageType): BrowserSidebarCategory[] {
		switch (packageType) {
			case 'mod':
				return this.mod();
			case 'datapack':
				return this.datapack();
			case 'resourcepack':
				return this.resourcepack();
			case 'shaderpack':
				return this.shaderpack();
		}

		return [];
	},

	mod(): BrowserSidebarCategory[] {
		return [
			{
				name: 'Categories',
				sub: [
					'adventure',
					'magic',
					'technology',
					'utility',
					'worldgen',
					'miscellaneous',
				],
			},
		];
	},

	datapack(): BrowserSidebarCategory[] {
		return [];
	},

	resourcepack(): BrowserSidebarCategory[] {
		return [];
	},

	shaderpack(): BrowserSidebarCategory[] {
		return [];
	},
};
