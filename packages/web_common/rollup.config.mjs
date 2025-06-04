import process from 'node:process';
import dts from 'rollup-plugin-dts';
import esbuild from 'rollup-plugin-esbuild';

/**
 * @param {string} id
 * @returns {boolean} true if the id is an external module
 */
const isExternal
	= process.platform === 'win32'
		? (/** @type {string} */ id) => !/^(?:[a-z]:\\|[.\\])/i.test(id)
		: (/** @type {string} */ id) => !/^[./]/.test(id);

/**
 * @param {string} input
 * @param {import('rollup').RollupOptions} config
 * @returns {import('rollup').RollupOptions} bundle options
 */
function bundle(input, config) {
	return {
		...config,
		input,
		external: isExternal,
	};
}

export default [
	// Output for NodeJS
	bundle('./src/index.ts', {
		plugins: [esbuild({ target: 'es2020' })],
		output: {
			file: `./lib/index.js`,
			format: 'esm',
			sourcemap: false,
			compact: false,
		},
	}),

	// Output for Typescript's .d.ts
	bundle('./src/index.ts', {
		plugins: [dts()],
		output: {
			file: `./lib/index.d.ts`,
			format: 'esm',
		},
	}),

	// Output for React components
	bundle('./src/components/index.ts', {
		plugins: [esbuild({ target: 'es2020' })],
		output: {
			file: `./lib/components.js`,
			format: 'esm',
			sourcemap: false,
			compact: false,
		},
		jsx: 'react-jsx',
	}),

	// Output for React components .d.ts
	bundle('./src/components/index.ts', {
		plugins: [dts()],
		output: {
			file: `./lib/components.d.ts`,
			format: 'esm',
		},
		jsx: 'react-jsx',
	}),

	// Output for browser
	// bundle({
	// 	plugins: [esbuild({ target: 'es2020', minify: true })],
	// 	output: {
	// 		file: `./dist/${name}-v${version}.js`,
	// 		format: 'esm',
	// 		name: camelCaseName,
	// 		sourcemap: true,
	// 		compact: true,
	// 	},
	// 	jsx: 'react-jsx',
	// }),
];
