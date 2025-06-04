import type { RefAttributes } from 'react';
import type { ButtonProps as AriaButtonProps } from 'react-aria-components';
import type { VariantProps } from 'tailwind-variants';
import { Button as AriaButton } from 'react-aria-components';
import { tv } from 'tailwind-variants';

export const buttonVariants = tv({
	base: [
		'flex flex-row justify-center items-center gap-x-2.5 rounded-lg disabled:pointer-events-none',
	],
	variants: {
		color: {
			primary: [
				'text-fg-primary hover:text-fg-primary-hover pressed:text-fg-primary-pressed disabled:text-fg-primary-disabled',
				'bg-brand hover:bg-brand-hover pressed:bg-brand-pressed disabled:bg-brand-disabled',
			],
			secondary: [
				'border box-border border-component-border hover:border-component-border-hover pressed:border-component-border-pressed',
				'text-fg-primary hover:text-fg-primary-hover pressed:text-fg-primary-pressed disabled:text-fg-primary-disabled',
				'bg-component-bg hover:bg-component-bg-hover active:bg-component-bg-pressed disabled:bg-component-bg-disabled',
			],
			danger: [
				'text-fg-primary hover:text-fg-primary-hover pressed:text-fg-primary-pressed disabled:text-fg-primary-disabled',
				'bg-danger hover:bg-danger-hover pressed:bg-danger-pressed disabled:bg-danger-disabled',
			],
			ghost: [
				'hover:bg-component-bg-hover pressed:bg-component-bg-pressed checked:selected:bg-component-bg-hover',
			],
		},
		size: {
			normal: 'py-1.5 px-3',
			icon: 'p-1.5 aspect-square box-border w-8 h-8',
			iconLarge: 'w-9 h-9 [&>*]:p-0.5',
		},
	},
});

export type ButtonVariantsProps = VariantProps<typeof buttonVariants>;

export type ButtonProps = AriaButtonProps & RefAttributes<HTMLButtonElement> & {
	color?: ButtonVariantsProps['color'];
	size?: ButtonVariantsProps['size'];
	className?: string;
	isDisabled?: boolean;
};

export function Button({
	color = 'primary',
	size = 'normal',
	className,
	...props
}: ButtonProps) {
	return (
		<AriaButton
			className={buttonVariants({ color, size, className })}
			isDisabled={props.isDisabled}
			{...props}
		/>
	);
}
