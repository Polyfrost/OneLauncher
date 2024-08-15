import { defineConfig } from '@flowr/eslint-config';

export default defineConfig({
	typescript: true,
	solid: true,
	unocss: true,
	toml: true,
	gitignore: true,
	schema: false,
	ignores: [
		'**/types',
		'**/cache',
		'**/*.svg',
		'**/gen',
		'*.rs',
	],
});
