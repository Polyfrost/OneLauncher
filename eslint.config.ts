import { defineConfig } from '@flowr/eslint-config';

export default defineConfig({
	typescript: true,
	solid: true,
	unocss: true,
	toml: true,
	gitignore: true,
	ignores: ['*.rs', '**/migrations'],
	rules: {
		'ts/no-use-before-define': 'off',
	},
});
