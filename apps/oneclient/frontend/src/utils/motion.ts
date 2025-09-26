import type { MotionProps } from 'motion/react';

export const transitions = {
	spring: {
		duration: 0.3,
		bounce: 0.1,
		power: 0.2,
		type: 'spring',
	},
} as const satisfies Record<string, MotionProps['transition']>;

export const animations = {
	slideInRight: {
		initial: {
			right: '-100%',
		},
		animate: {
			right: '0',
		},
		transition: transitions.spring,
	},

	slideInLeft: {
		initial: {
			position: 'relative',
			left: '-100%',
		},
		animate: {
			position: 'relative',
			left: '0',
		},
	},

	slideInUp: {
		initial: {
			position: 'relative',
			bottom: '-100%',
		},
		animate: {
			position: 'relative',
			bottom: '0',
		},
	},

	fadeIn: {
		initial: {
			opacity: 0,
		},
		animate: {
			opacity: 1,
		},
	},
} as const satisfies Record<string, MotionProps>;
