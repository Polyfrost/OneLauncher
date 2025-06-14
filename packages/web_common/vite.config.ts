import path from 'node:path';
import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';
import dts from 'vite-plugin-dts';
import { viteStaticCopy } from 'vite-plugin-static-copy';

export default defineConfig({
	plugins: [
		react(),
		tailwindcss(),
		viteStaticCopy({
			targets: [
				{
					src: path.resolve(__dirname, 'src/theme.css'),
					dest: './',
				},
			],
		}),
		dts({
			insertTypesEntry: true,
		}),
	],
	build: {
		minify: false,
		lib: {
			entry: {
				'index': path.resolve(__dirname, 'src/index.ts'),
				'components': path.resolve(__dirname, 'src/components/index.ts'),
			},
			formats: ['es'],
			fileName: (_, entryName) => `${entryName}.js`,
		},
		rollupOptions: {
			external: [
				'react',
				'react/jsx-runtime',
				'react-dom',
				'react-aria-components',
				'motion',
				'motion/react',
				'@tanstack/react-query',
				'@tanstack/react-router',
				'tailwindcss',
				'tailwind-merge',
				'tailwind-variants',
				// 'tailwindcss-animated',
				// 'tailwindcss-react-aria-components',
			],
			output: {
				globals: {
					'react': 'React',
					'react/jsx-runtime': 'react/jsx-runtime',
					'react-dom': 'ReactDOM',
					'tailwindcss': 'tailwindcss',
				},
			},
		},
	},
});
