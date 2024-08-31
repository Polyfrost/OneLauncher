import {
	defineConfig,
	presetAttributify,
	presetIcons,
	presetUno,
	transformerDirectives,
	transformerVariantGroup,
} from 'unocss';

export default defineConfig({
	rules: [],
	shortcuts: {},
	presets: [
		presetUno(),
		presetAttributify(),
		presetIcons(),
	],
	transformers: [
		transformerVariantGroup(),
		transformerDirectives(),
	],
	theme: {
		fontFamily: {
			sans: 'Poppins',
		},

		borderRadius: {
			md: '0.438rem', // 7px
		},

		fontSize: {
			// Based off 16px
			'xs': '0.688rem',
			'sm': '0.75rem',
			'md': '0.875rem',
			'lg': '1rem',
			'2lg': '1.125rem',
			'xl': '1.25rem',
			'xxl': '1.5rem',
			'2xl': '2rem',
			'3xl': '2.5rem',
			'4xl': '3rem',
		},

		colors: {
			white: '#ffffff',
			black: '#000000',

			gray: {
				'05': '#FFFFFF0D',
				'10': '#FFFFFF1A',
			},

			fg: {
				primary: {
					DEFAULT: '#D5DBFF',
					hover: '#D5DBFFD9',
					pressed: '#E1E5FF',
					disabled: '#E1E5FF80',
				},
				secondary: {
					DEFAULT: '#78818D',
					hover: '#5F6874',
					pressed: '#828D9B',
					disabled: '#78818D80',
				},
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

			success: {
				DEFAULT: '#239A60',
				hover: '#1A8752',
				pressed: '#2CAC6E',
				disabled: '#239A6080',
			},

			component: {
				bg: {
					DEFAULT: '#1A2229',
					hover: '#171F25',
					pressed: '#222C35',
					disabled: '#1A222980',
				},
			},

			code: {
				info: '#61AFEF',
				warn: '#E5C07B',
				error: '#BE5046',
				debug: '#2B4BFF',
				trace: '#FDFDFD',
			},

			link: {
				DEFAULT: '#61AFEF',
				hover: '#5F87FF',
				pressed: '#72AFFF',
				disabled: '#61AFEF80',
			},

			page: {
				DEFAULT: '#11171c',
				elevated: '#151c22',
				pressed: '#0e1317',
			},

			secondary: '#192026',
		},
		extend: {
			height: {
				15: '60px',
			},

			strokeWidth: {
				3: '3',
			},

			backgroundColor: {
				transparent: 'transparent',
			},

			// animation: {
			// 	'fade-in': 'fadeIn 150ms easeOutExpo forwards',
			// 	'fade-out': 'fadeOut 150ms easeOutExpo forwards',
			// },

			// keyframes: {
			// 	fadeIn: {
			// 		'0%': { opacity: '0' },
			// 		'100%': { opacity: '1' },
			// 	},
			// 	fadeOut: {
			// 		'0%': { opacity: '1' },
			// 		'100%': { opacity: '0' },
			// 	},
			// },
		},
	},
});
