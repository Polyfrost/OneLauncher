import { defineConfig } from '@flowr/eslint';
// @ts-ignore -- No types for this package
import { tanstackConfig } from '@tanstack/eslint-config';

export default defineConfig(
	{
		typescript: true,
		react: true,
		query: true,
		ignores: ['*.rs', '**/migrations/**'],
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
	{
		ignores: [
			'apps/onelauncher/old_frontend',
			'apps/*/desktop/capabilities',
			'apps/*/desktop/gen',
			'**/*.gen.*',
		],
		// overrides: [
		// 	{
		// 		files: ['vite.config.js'],
		// 		parserOptions: {
		// 			project: null, // disables type-aware linting for this file
		// 		},
		// 	},
		// ],
	},
	{
		rules: {
			'sort-imports': 'off', // Replaced by perfectionist/sort-named-imports',
			'import/order': 'off', // Replaced by perfectionist/sort-named-imports',
			'style/jsx-max-props-per-line': ['error', { maximum: 3 }],
			'ts/no-use-before-define': 'off',
			'style/function-paren-newline': ['error', 'consistent'],
			'react/no-context-provider': 'off',
			'prefer-const': 'off', // disabled due to maximum call stack size,
			'style/jsx-one-expression-per-line': ['error', { allow: 'non-jsx' }],
			'unused-imports/no-unused-imports': 'off',
			'eslint-comments/no-unlimited-disable': 'off',
			'eslint-comments/require-description': ['error', {
				ignore: [
					'eslint-disable',
				],
			}],
			'no-shadow': 'off',
		},
	},
);
