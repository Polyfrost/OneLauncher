import type { HTMLAttributes } from 'react';
import type { VariantProps } from 'tailwind-variants';
import { tv } from 'tailwind-variants';

const spinnerVariants = tv({
	base: 'inline-block relative opacity-0 animate-delay-1000 animate-fade animate-fill-forwards',
	variants: {
		size: {
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
		<div className={spinnerVariants({ size, className })}>
			<div className="absolute w-full h-full rounded-full border-2 border-fg-primary animate-fade-scale"></div>
			<div className="absolute w-full h-full rounded-full border-2 border-fg-primary animate-fade-scale animate-delay-500"></div>
		</div>
	);
}
