import { Input } from 'react-aria-components';
import type { RefAttributes } from 'react';
import type { InputProps as AriaInputProps } from 'react-aria-components';
import type { VariantProps } from 'tailwind-variants';
import { tv } from 'tailwind-variants';

export const textFieldVariants = tv({
	base: [
		'bg-component-bg',
		'[&:not(:focus-within)]:hover:bg-component-bg-hover',
		'focus-within:bg-component-bg-pressed',
		'disabled:bg-component-bg-disabled',
		'border-component-border border fill-fg-primary',
		'rounded-lg px-2.5 py-1.5 min-h-8',
		'flex flex-row items-center justify-start gap-x-1.5',
		'text-sm text-fg-primary font-medium',
		'bg-transparent outline-none appearance-none p-0 m-0 box-border w-full font-medium',
		'[&::selection]:bg-brand/30',
		'placeholder:text-fg-secondary/40 placeholder:font-normal'
	],
	variants: {
		invalid: {
			true: {
				base: 'border-danger'
			}
		}
	}
})

export type TextFieldVariants = VariantProps<typeof textFieldVariants>;

export type TextFieldProps = AriaInputProps & RefAttributes<HTMLInputElement> & {
	className?: string;
};

export function TextField({
	className,
	...props
}: TextFieldProps) {
	return (
		<>
			<Input className={textFieldVariants({ className })} {...props} />
		</>
	)
}