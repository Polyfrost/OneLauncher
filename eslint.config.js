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
		'**/.temp',
		'**/*.svg',
		'**/gen',
		'*.rs',
		'pnpm-lock.yaml',
		'**/node_modules',
		'apps/desktop/src/bindings.ts',
	],
});
