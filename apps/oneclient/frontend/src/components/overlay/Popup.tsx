import type { PopoverProps } from 'react-aria-components';
import { Dialog, DialogTrigger, Popover } from 'react-aria-components';
import { twMerge } from 'tailwind-merge';
import { tv } from 'tailwind-variants';

export interface PopupProps extends Omit<PopoverProps, 'children'> {
	children: React.ReactNode;
	className?: string | undefined;
}

const popupVariants = tv({
	slots: {
		popover: [
			'flex flex-col gap-0.5 rounded-lg',

			'entering:animate-fade entering:animate-duration-75',
			'exiting:animate-fade exiting:animate-duration-75 exiting:animate-reverse',
		],
		dialog: [
			'flex flex-col items-center p-1 gap-2 relative',
			'border border-component-border-hover bg-page-elevated shadow-md rounded-lg',
		],
	},
});

export function Popup({
	children,
	className,
	...props
}: PopupProps) {
	const { popover, dialog } = popupVariants();

	return (
		<Popover {...props} className={twMerge(popover(), className)}>
			<Dialog className={dialog()} role="dialog">
				{children}
			</Dialog>
		</Popover>
	);
}

Popup.Trigger = DialogTrigger;
