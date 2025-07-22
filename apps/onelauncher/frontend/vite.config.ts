import { resolve } from 'node:path';
import process from 'node:process';
import tailwindcss from '@tailwindcss/vite';

import { TanStackRouterVite } from '@tanstack/router-plugin/vite';
import viteReact from '@vitejs/plugin-react';

import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [
		TanStackRouterVite({ autoCodeSplitting: true }),
		viteReact(),
		tailwindcss(),
	],
	clearScreen: false,
	server: {
		port: 8001,
		strictPort: true,
	},
	resolve: {
		alias: {
			'@': resolve(__dirname, './src'),
		},
	},
	envPrefix: ['VITE_', 'TAURI_ENV_'],
	build: {
		target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
		minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
		sourcemap: !!process.env.TAURI_ENV_DEBUG,
	},
});
