/// <reference types="vitest/config"/>

import process from 'node:process';
import type { Plugin, UserConfig } from 'vite';
import { defineConfig, loadEnv } from 'vite';

import solid from 'vite-plugin-solid';
import paths from 'vite-tsconfig-paths';
import unocss from 'unocss/vite';

export default defineConfig(async ({ mode }) => {
	process.env = { ...process.env, ...loadEnv(mode, process.cwd(), '') };

	const config: UserConfig = {
		plugins: [
			unocss(),
			solid(),
			paths(),
			devtools(),
		],

		envPrefix: ['VITE_', 'TAURI_'],

		build: {
			target: ['esnext', 'chrome110', 'safari13'],
			minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
			sourcemap: !!process.env.TAURI_DEBUG,
		},

		clearScreen: false,
		server: {
			port: 8001,
			strictPort: true,
			watch: {
				ignored: ['**/src-tauri/**'],
			},
		},

		test: {
			globals: true,
			reporters: ['dot'],
		},
	};

	return config;
});

function devtools(): Plugin {
	return {
		name: 'devtools-plugin',
		transformIndexHtml(html) {
			if (process.env.NODE_ENV === 'development') {
				const devtoolsScript = `<script src ="http://localhost:8087"></script>`;
				const headTagIndex = html.indexOf('</head>');
				if (headTagIndex > -1)
					return html.slice(0, headTagIndex) + devtoolsScript + html.slice(headTagIndex);
			}

			return html;
		},
	};
}
