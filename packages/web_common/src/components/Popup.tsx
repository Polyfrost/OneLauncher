import type { PopoverProps } from 'react-aria-components';
import { Dialog, DialogTrigger, Popover } from 'react-aria-components';

interface MyPopoverProps extends Omit<PopoverProps, 'children'> {
	children: React.ReactNode;
}

export function Popup({ children, ...props }: MyPopoverProps) {
	return (
		<Popover {...props} className="flex flex-col gap-0.5 rounded-lg border border-component-border-hover">
			<Dialog className="bg-page-elevated p-2 shadow-md rounded-lg">
				{children}
			</Dialog>
		</Popover>
	);
}

function Trigger({ children, ...props }: any) {
	return <DialogTrigger children={children} {...props} />;
}

Popup.Trigger = Trigger;
