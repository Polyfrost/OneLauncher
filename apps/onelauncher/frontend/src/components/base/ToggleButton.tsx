import type { RefAttributes } from 'react';
import type { ToggleButtonProps as AriaToggleButtonProps } from 'react-aria-components';
import type { ButtonVariants } from './Button';
import { ToggleButton as AriaToggleButton } from 'react-aria-components';
import { buttonVariants } from './Button';

export type ToggleButtonProps = AriaToggleButtonProps & RefAttributes<HTMLButtonElement> & {
	color?: ButtonVariants['color'];
	size?: ButtonVariants['size'];
	className?: string;
};

function ToggleButton({
	color = 'ghost',
	size = 'normal',
	className,
	...props
}: ToggleButtonProps) {
	return (
		<AriaToggleButton className={buttonVariants({ color, size, className })} {...props} />
	);
}

export default ToggleButton;
