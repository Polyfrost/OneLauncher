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
			white: 'rgba(var(--clr-white), <alpha-value>)',
			black: 'rgba(var(--clr-black), <alpha-value>)',

			border: 'rgba(var(--clr-border), <alpha-value>)',

			fg: {
				primary: {
					DEFAULT: 'rgba(var(--clr-fg-primary), <alpha-value>)',
					hover: 'rgba(var(--clr-fg-primary-hover), <alpha-value>)',
					pressed: 'rgba(var(--clr-fg-primary-pressed), <alpha-value>)',
					disabled: 'rgba(var(--clr-fg-primary-disabled), <alpha-value>)',
				},
				secondary: {
					DEFAULT: 'rgba(var(--clr-fg-secondary), <alpha-value>)',
					hover: 'rgba(var(--clr-fg-secondary-hover), <alpha-value>)',
					pressed: 'rgba(var(--clr-fg-secondary-pressed), <alpha-value>)',
					disabled: 'rgba(var(--clr-fg-secondary-disabled), <alpha-value>)',
				},
			},

			brand: {
				DEFAULT: 'rgba(var(--clr-brand), <alpha-value>)',
				hover: 'rgba(var(--clr-brand-hover), <alpha-value>)',
				pressed: 'rgba(var(--clr-brand-pressed), <alpha-value>)',
				disabled: 'rgba(var(--clr-brand-disabled), <alpha-value>)',
			},

			onbrand: {
				DEFAULT: 'rgba(var(--clr-onbrand), <alpha-value>)',
				hover: 'rgba(var(--clr-onbrand-hover), <alpha-value>)',
				pressed: 'rgba(var(--clr-onbrand-pressed), <alpha-value>)',
				disabled: 'rgba(var(--clr-onbrand-disabled), <alpha-value>)',
			},

			danger: {
				DEFAULT: 'rgba(var(--clr-danger), <alpha-value>)',
				hover: 'rgba(var(--clr-danger-hover), <alpha-value>)',
				pressed: 'rgba(var(--clr-danger-pressed), <alpha-value>)',
				disabled: 'rgba(var(--clr-danger-disabled), <alpha-value>)',
			},

			success: {
				DEFAULT: 'rgba(var(--clr-success), <alpha-value>)',
				hover: 'rgba(var(--clr-success-hover), <alpha-value>)',
				pressed: 'rgba(var(--clr-success-pressed), <alpha-value>)',
				disabled: 'rgba(var(--clr-success-disabled), <alpha-value>)',
			},

			component: {
				bg: {
					DEFAULT: 'rgba(var(--clr-component-bg), <alpha-value>)',
					hover: 'rgba(var(--clr-component-bg-hover), <alpha-value>)',
					pressed: 'rgba(var(--clr-component-bg-pressed), <alpha-value>)',
					disabled: 'rgba(var(--clr-component-bg-disabled), <alpha-value>)',
				},
			},

			code: {
				info: 'rgba(var(--clr-code-info), <alpha-value>)',
				warn: 'rgba(var(--clr-code-warn), <alpha-value>)',
				error: 'rgba(var(--clr-code-error), <alpha-value>)',
				debug: 'rgba(var(--clr-code-debug), <alpha-value>)',
				trace: 'rgba(var(--clr-code-trace), <alpha-value>)',
			},

			link: {
				DEFAULT: 'rgba(var(--clr-link), <alpha-value>)',
				hover: 'rgba(var(--clr-link-hover), <alpha-value>)',
				pressed: 'rgba(var(--clr-link-pressed), <alpha-value>)',
				disabled: 'rgba(var(--clr-link-disabled))',
			},

			page: {
				DEFAULT: 'rgba(var(--clr-page), <alpha-value>)',
				elevated: 'rgba(var(--clr-page-elevated), <alpha-value>)',
				pressed: 'rgba(var(--clr-page-pressed), <alpha-value>)',
			},

			secondary: 'rgba(var(--clr-secondary), <alpha-value>)',
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
		},
	},
});
