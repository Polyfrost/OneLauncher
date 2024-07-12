// @ts-check
import petal from '@flowr/eslint-config';

export default petal({
	typescript: true,
	solid: true,
	unocss: false,
	toml: {
		overrides: {
			'toml/padding-line-between-pairs': 'off',
		},
	},
	ignores: [
		'**/target',
		'**/dist',
		'**/types',
		'**/cache',
		'**/dist',
		'**/.temp',
		'**/*.svg',
		'**/gen',
		'*.rs',
		'pnpm-lock.yaml',
		'**/node_modules',
		'desktop/src/bindings.ts',
	],
}).append([
	{
		rules: {
			'petal/consistent-list-newline': 'off',
			'no-console': 'off',
			'new-cap': 'off',
			// temporarily off
			'unused-imports/no-unused-vars': 'off',
		},
	},
]);
