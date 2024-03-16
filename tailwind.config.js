/** @type {import('tailwindcss').Config} */
export default {
    content: ['./src/**/*.{js,ts,jsx,tsx}'],
    theme: {
        fontSize: {
            // Based off 16px
            sm: '0.688rem',
            md: '0.875rem',
            lg: '1rem',
            xl: '1.25rem',
            xxl: '1.5rem',
            '2xl': '2rem',
        },
        colors: {
            white: '#ffffff',
            black: '#000000',
            slate: '#9BA1A6',

            gray: {
                400: '#D5DBFF',
                '.5': '#FFFFFF0D',
                '.10': '#FFFFFF1A',
            },

            brand: {
                DEFAULT: '#2B4BFF',
                hover: '#2843DD',
                pressed: '#3957FF',
                disabled: '#3957FF80',
            },

            onbrand: {
                DEFAULT: '#D5DBFF',
                hover: '#D5DBFFD9',
                pressed: '#E1E5FF',
                disabled: '#E1E5FF80',
            },

            danger: {
                DEFAULT: '#FF4444',
                hover: '#D63434',
                pressed: '#FF5656',
                disabled: '#FF444480',
            },

            component: {
                bg: {
                    DEFAULT: '#1A2229',
                    hover: '#171F25',
                    pressed: '#222C35',
                    disabled: '#1A222980',
                },
            },

            primary: '#11171C',
            secondary: '#192026',
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
