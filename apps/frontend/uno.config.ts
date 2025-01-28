import {
	defineConfig,
	presetIcons,
	presetUno,
	transformerDirectives,
	transformerVariantGroup,
} from 'unocss';

export default defineConfig({
	rules: [],
	shortcuts: {},
	presets: [
		presetUno({
			dark: {
				dark: 'body[data-theme-type="dark"]',
				light: 'body[data-theme-type="light"]',
			},
		}),
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
			white: 'rgba(var(--clr-white))',
			black: 'rgba(var(--clr-black))',

			border: 'rgba(var(--clr-border))',

			fg: {
				primary: {
					DEFAULT: 'rgba(var(--clr-fg-primary))',
					hover: 'rgba(var(--clr-fg-primary-hover))',
					pressed: 'rgba(var(--clr-fg-primary-pressed))',
					disabled: 'rgba(var(--clr-fg-primary-disabled))',
				},
				secondary: {
					DEFAULT: 'rgba(var(--clr-fg-secondary))',
					hover: 'rgba(var(--clr-fg-secondary-hover))',
					pressed: 'rgba(var(--clr-fg-secondary-pressed))',
					disabled: 'rgba(var(--clr-fg-secondary-disabled))',
				},
			},

			brand: {
				DEFAULT: 'rgba(var(--clr-brand))',
				hover: 'rgba(var(--clr-brand-hover))',
				pressed: 'rgba(var(--clr-brand-pressed))',
				disabled: 'rgba(var(--clr-brand-disabled))',
			},

			onbrand: {
				DEFAULT: 'rgba(var(--clr-onbrand))',
				hover: 'rgba(var(--clr-onbrand-hover))',
				pressed: 'rgba(var(--clr-onbrand-pressed))',
				disabled: 'rgba(var(--clr-onbrand-disabled))',
			},

			danger: {
				DEFAULT: 'rgba(var(--clr-danger))',
				hover: 'rgba(var(--clr-danger-hover))',
				pressed: 'rgba(var(--clr-danger-pressed))',
				disabled: 'rgba(var(--clr-danger-disabled))',
			},

			success: {
				DEFAULT: 'rgba(var(--clr-success))',
				hover: 'rgba(var(--clr-success-hover))',
				pressed: 'rgba(var(--clr-success-pressed))',
				disabled: 'rgba(var(--clr-success-disabled))',
			},

			component: {
				bg: {
					DEFAULT: 'rgba(var(--clr-component-bg))',
					hover: 'rgba(var(--clr-component-bg-hover))',
					pressed: 'rgba(var(--clr-component-bg-pressed))',
					disabled: 'rgba(var(--clr-component-bg-disabled))',
				},
			},

			code: {
				info: 'rgba(var(--clr-code-info))',
				warn: 'rgba(var(--clr-code-warn))',
				error: 'rgba(var(--clr-code-error))',
				debug: 'rgba(var(--clr-code-debug))',
				trace: 'rgba(var(--clr-code-trace))',
			},

			link: {
				DEFAULT: 'rgba(var(--clr-link))',
				hover: 'rgba(var(--clr-link-hover))',
				pressed: 'rgba(var(--clr-link-pressed))',
				disabled: 'rgba(var(--clr-link-disabled))',
			},

			page: {
				DEFAULT: 'rgba(var(--clr-page))',
				elevated: 'rgba(var(--clr-page-elevated))',
				pressed: 'rgba(var(--clr-page-pressed))',
			},

			secondary: 'rgba(var(--clr-secondary))',
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
