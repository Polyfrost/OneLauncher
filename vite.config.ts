import process from 'node:process';
import type { Plugin } from 'vite';
import { defineConfig, loadEnv } from 'vite';

import solid from 'vite-plugin-solid';
// idk if we are going to want to use sentry since we would have to selfhost
// we also need to consider self hosting a translation service
// and figuring out a way to implement analytics into this
import { sentryVitePlugin } from '@sentry/vite-plugin';

// hopefully once we merge into nexus monorepo this can be one standard vite config
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

	const config = {
		plugins: [
			solid(),
			devtoolsPlugin,
		],

		clearScreen: false,
		server: {
			port: 1420,
			strictPort: true,
			watch: {
				ignored: ['**/src-tauri/**'],
			},
		},
	};

	if (process.env.SENTRY_AUTH_TOKEN) {
		config.plugins.push(sentryVitePlugin({
			authToken: process.env.SENTRY_AUTH_TOKEN,
			url: 'https://sentry.polyfrost.org',
			org: 'polyfrost',
			project: 'launcher',
		}));
	}

	return config;
});
