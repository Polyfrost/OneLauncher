import type { JSX, RefAttributes } from 'react';
import type { InputProps as AriaInputProps } from 'react-aria-components';
import type { VariantProps } from 'tailwind-variants';
import { TextField as AriaTextField, Input } from 'react-aria-components';
import { tv } from 'tailwind-variants';

export const textFieldVariants = tv({
	slots: {
		container: [
			'bg-component-bg',
			'[&:not(:focus-within)]:hover:bg-component-bg-hover',
			'focus-within:bg-component-bg-pressed',
			'border-component-border border',
			'rounded-lg px-2.5 py-1.5 min-h-8',
			'flex flex-row items-center justify-start gap-x-1.5',
			'text-sm text-fg-primary font-medium',
		],
		input: [
			'outline-none appearance-none bg-transparent',
			'text-sm text-fg-primary font-medium',
			'flex-1 min-w-0',
			'[&::selection]:bg-brand/30',
			'placeholder:text-fg-secondary/40 placeholder:font-normal',
		],
		icon: [
			'text-fg-secondary flex-shrink-0',
		],
	},
	variants: {
		invalid: {
			true: {
				container: 'border-danger',
			},
		},
	},
});

export type TextFieldVariantsProps = VariantProps<typeof textFieldVariants>;

export type TextFieldProps = AriaInputProps & RefAttributes<HTMLInputElement> & {
	className?: string;
	iconLeft?: JSX.Element;
	iconRight?: JSX.Element;
};

export function TextField({
	className,
	iconLeft,
	iconRight,
	...props
}: TextFieldProps) {
	const styles = textFieldVariants();

	return (
		<AriaTextField className={styles.container({ className })}>
			{iconLeft && (
				<span className={styles.icon()}>
					{iconLeft}
				</span>
			)}
			<Input className={styles.input()} {...props} />
			{iconRight && (
				<span className={styles.icon()}>
					{iconRight}
				</span>
			)}
		</AriaTextField>
	);
}
