/// <reference types="vitest/config"/>

import process from 'node:process';
import type { UserConfig } from 'vite';
import { defineConfig, loadEnv } from 'vite';

import solid from 'vite-plugin-solid';
import solidSvg from 'vite-plugin-solid-svg';
import paths from 'vite-tsconfig-paths';
import unocss from 'unocss/vite';

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
