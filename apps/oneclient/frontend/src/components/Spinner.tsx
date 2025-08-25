import type { HTMLAttributes } from 'react';
import type { VariantProps } from 'tailwind-variants';
import { tv } from 'tailwind-variants';
import styles from './Spinner.module.css';

const spinnerVariants = tv({
	base: styles.loader,
	variants: {
		size: {
			extraSmall: 'w-6 h-6',
			small: 'w-8 h-8',
			medium: 'w-12 h-12',
			large: 'w-16 h-16',
		},
	},
});

type SpinnerVariantsProps = VariantProps<typeof spinnerVariants>;

export interface SpinnerProps extends HTMLAttributes<HTMLDivElement> {
	size?: SpinnerVariantsProps['size'];
};

export function Spinner({
	size = 'medium',
	className,
}: SpinnerProps) {
	return (
		<div className={spinnerVariants({ size, className })}></div>
	);
}
