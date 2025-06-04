import type { RefAttributes } from 'react';
import type { ToggleButtonProps as AriaToggleButtonProps } from 'react-aria-components';
import type { ButtonVariantsProps } from './Button';
import { ToggleButton as AriaToggleButton } from 'react-aria-components';
import { buttonVariants } from './Button';

export type ToggleButtonProps = AriaToggleButtonProps & RefAttributes<HTMLButtonElement> & {
	color?: ButtonVariantsProps['color'];
	size?: ButtonVariantsProps['size'];
	className?: string;
};

export function ToggleButton({
	color = 'ghost',
	size = 'normal',
	className,
	...props
}: ToggleButtonProps) {
	return (
		<AriaToggleButton className={buttonVariants({ color, size, className })} {...props} />
	);
}
