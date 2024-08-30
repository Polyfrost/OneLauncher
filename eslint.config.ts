import { defineConfig } from '@flowr/eslint-config';

export default defineConfig({
	typescript: true,
	solid: true,
	unocss: true,
	ignores: ['*.rs', '**/migrations/**'],
	rules: {
		'ts/no-use-before-define': 'off',
	},
});
