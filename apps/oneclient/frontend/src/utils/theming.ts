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

export interface Theme {
	variants: Array<ThemeVariant>;
}

export type ThemeTypes = 'dark' | 'light';

export interface ThemeVariant {
	type: ThemeTypes;
	name: string;
};
