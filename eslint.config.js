// @ts-check
import { defineConfig } from '@flowr/eslint-config';

export default defineConfig({
	typescript: true,
	solid: true,
	unocss: false,
	toml: {
		overrides: {
			'toml/padding-line-between-pairs': 'off',
		},
	},
	gitignore: true,
	ignores: [
		'**/target',
		'**/dist',
		'**/types',
		'**/cache',
		'**/.temp',
		'**/*.svg',
		'**/gen',
		'*.rs',
		'pnpm-lock.yaml',
		'**/node_modules',
	],
});
