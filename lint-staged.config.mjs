// @ts-check

/**
 * @type {import('lint-staged').Configuration}
 */
const config = {
	'*.ts': 'eslint',
	'*.{js,mjs,cjs}': 'eslint', // handle JS, MJS, CJS in one fell swoop
	'*.rs': [
		'rustfmt', // use rustfmt to only check the files provided by lint-staged
		'cargo clippy -- -D warnings', // sadly clippy can't do this
	],
};
export default config;
