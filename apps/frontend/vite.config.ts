import type { UserConfig } from 'vite';
import process from 'node:process';
import { sentryVitePlugin as sentry } from '@sentry/vite-plugin';

import unocss from 'unocss/vite';
import { defineConfig, loadEnv } from 'vite';
import solid from 'vite-plugin-solid';
import solidSvg from 'vite-plugin-solid-svg';
import paths from 'vite-tsconfig-paths';

export default defineConfig(async ({ mode }) => {
	process.env = { ...process.env, ...loadEnv(mode, process.cwd(), '') };

	const config: UserConfig = {
		plugins: [
			unocss(),
			solid(),
			solidSvg({
				defaultAsComponent: false,
			}),
			paths(),
		],

		envPrefix: ['VITE_', 'TAURI_'],

		build: {
			target: ['esnext', 'chrome110', 'safari13'],
			minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
			sourcemap: !!process.env.TAURI_DEBUG,
			rollupOptions: {
				treeshake: 'recommended',
			},
			emptyOutDir: false,
		},

		css: {
			preprocessorOptions: {
				scss: {
					api: 'modern-compiler',
				},
			},
		},

		clearScreen: false,
		server: {
			port: 8001,
			strictPort: true,
		},
	};

	if (process.env.SENTRY_AUTH_TOKEN)
		config.plugins.push(sentry({
			authToken: process.env.SENTRY_AUTH_TOKEN,
			org: 'polyfrost',
			project: 'onelauncher_frontend',
			url: 'https://sentry.polyfrost.org',
			telemetry: false,
		}));

	return config;
});
