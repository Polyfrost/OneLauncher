import process from 'node:process';
import type { Plugin, UserConfig } from 'vite';
import { defineConfig, loadEnv } from 'vite';

import solid from 'vite-plugin-solid';

const devtoolsPlugin: Plugin = {
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

export default defineConfig(async ({ mode }) => {
	process.env = { ...process.env, ...loadEnv(mode, process.cwd(), '') };

	const config: UserConfig = {
		plugins: [
			solid(),
			devtoolsPlugin,
		],

		envPrefix: ['VITE_', 'TAURI_'],

		build: {
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
	};

	return config;
});
