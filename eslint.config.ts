import { defineConfig } from '@flowr/eslint-config';

export default defineConfig({
	typescript: true,
	solid: true,
	unocss: false,
	toml: true,
	gitignore: true,
	ignores: [
		'**/types',
		'**/cache',
		'**/*.svg',
		'**/gen',
		'*.rs',
	],
});
