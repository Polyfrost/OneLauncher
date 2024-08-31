import type { BrowserListView, PackageType } from '@onelauncher/client/bindings';
import type { BrowserSidebarCategory } from '~ui/pages/browser/BrowserRoot';
import { LOADERS, PROVIDERS } from '~utils';

export const BROWSER_VIEWS: BrowserListView[] = ['grid', 'list', 'preview'] as const;

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
	},

	mod(): BrowserSidebarCategory[] {
		return [
			{
				name: 'Categories',
				sub: [
					['Adventure', '/adventure'],
					['Magic', '/magic'],
					['Technology', '/technology'],
					['Utility', '/utility'],
					['Worldgen', '/worldgen'],
					['Miscellaneous', '/miscellaneous'],
				],
			},
			...this.loader(false),
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

	provider(): BrowserSidebarCategory[] {
		return [
			{
				name: 'Providers',
				sub: PROVIDERS.map(provider => [provider, `/${provider.toLowerCase()}`]),
			},
		];
	},

	loader(vanilla: boolean = false): BrowserSidebarCategory[] {
		const loaders: [string, string][] = [];

		for (const loader of LOADERS) {
			if (vanilla === false && loader === 'vanilla')
				continue;

			loaders.push([loader, `/${loader.toLowerCase()}`]);
		}

		return [
			{
				name: 'Loaders',
				sub: loaders,
			},
		];
	},
};
