import { defineConfig } from '@flowr/eslint';
// @ts-ignore -- No types for this package
import { tanstackConfig } from '@tanstack/eslint-config';

export default defineConfig(
	{
		typescript: true,
		react: true,
		query: true,
		ignores: ['*.rs', '**/migrations/**'],
		rules: {
			'ts/no-use-before-define': 'off',
		},
		toml: {
			overrides: {
				'toml/array-element-newline': ['error', 'always'],
				'toml/no-mixed-type-in-array': ['off'],
			},
		},
	},
	// Hack to fix an eslint error with plugin names
	// @ts-ignore -- No types for this package
	tanstackConfig.map((config) => {
		if (config.rules && config.rules['@stylistic/js/spaced-comment'])
			delete config.rules['@stylistic/js/spaced-comment'];

		return config;
	}),
);
