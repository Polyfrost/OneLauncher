/* eslint-disable import/no-extraneous-dependencies */
import postcssEasings from 'postcss-easings';
import tailwind from 'tailwindcss';
import autoprefixer from 'autoprefixer';

export default {
    plugins: [
        tailwind,
        autoprefixer,
        postcssEasings,
    ],
};
