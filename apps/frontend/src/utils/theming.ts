// When adding a theme, make sure to add the theme here and define the colors in the CSS file!
export const DEFAULT_THEME = 'OneLauncher-Dark';
export const THEMES = {
	OneLauncher: {
		variants: [
			{
				name: 'Dark',
				type: 'dark',
			},
			{
				name: 'Light',
				type: 'light',
			},
		],
	},
} as const satisfies Record<string, Theme>;

export function setAppTheme(name: string, variant: ThemeVariant) {
	document.body.setAttribute('data-theme', `${name}-${variant.name}`);
	document.body.setAttribute('data-theme-type', variant.type);
}

export function splitMergedTheme(merged: string): {
	theme: keyof typeof THEMES;
	variant: ThemeVariant;
} {
	const [name, ...variants] = merged.split('-');

	const foundVariant = THEMES[name as keyof typeof THEMES].variants.find(variant => variant.name === variants.join('-'));

	if (!name || !foundVariant)
		return {
			theme: 'OneLauncher',
			variant: THEMES.OneLauncher.variants[0],
		};

	return {
		theme: name as keyof typeof THEMES,
		variant: foundVariant,
	};
}

export interface Theme {
	variants: ThemeVariant[];
}

export type ThemeTypes = 'dark' | 'light';

export interface ThemeVariant {
	type: ThemeTypes;
	name: string;
};
