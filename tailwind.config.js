/** @type {import('tailwindcss').Config} */
export default {
    content: ['./src/**/*.{js,ts,jsx,tsx}'],
    theme: {
        colors: {
            white: '#ffffff',
            black: '#000000',
            slate: '#9BA1A6',
            danger: '#E5484D',
            bg: {
                primary: '#11171C',
                secondary: '#192026',
            },
        },
        extend: {
            animation: {
                'fade-in': 'fadeIn 150ms easeOutExpo forwards',
                'fade-out': 'fadeOut 150ms easeOutExpo forwards',
            },

            keyframes: {
                fadeIn: {
                    '0%': { opacity: '0' },
                    '100%': { opacity: '1' },
                },
                fadeOut: {
                    '0%': { opacity: '1' },
                    '100%': { opacity: '0' },
                },
            },
        },
    },
    plugins: [],
};
