import postcssEasings from 'postcss-easings';
import tailwind from 'tailwindcss';
import autoprefixer from 'autoprefixer';

const config = {
	plugins: [
		tailwind,
		autoprefixer,
		postcssEasings,
	],
};

export default config;
