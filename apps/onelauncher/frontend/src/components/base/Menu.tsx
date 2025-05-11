import type { ClassNameString } from '@/types/global';
import type { MenuItemProps as AriaMenuItemProps, MenuProps as AriaMenuProps, PopoverProps as AriaPopoverProps } from 'react-aria-components';
import { Menu as AriaMenu, MenuItem as AriaMenuItem, Popover as AriaPopover } from 'react-aria-components';
import { twMerge } from 'tailwind-merge';

function Menu<T extends object>({
	className,
	...rest
}: AriaMenuProps<T> & ClassNameString) {
	return (
		<AriaMenu
			className={twMerge(
				'flex flex-col gap-0.5 rounded-lg border border-component-border bg-page-elevated p-2 shadow-md',
				className,
			)}
			{...rest}
		/>
	);
}

export default Menu;

Menu.Popover = ({
	className,
	...rest
}: AriaPopoverProps & ClassNameString) => (
	<AriaPopover
		className={twMerge('animate-fade animate-duration-75', className)}
		{...rest}
	/>
);

Menu.Item = ({
	className,
	...rest
}: AriaMenuItemProps & ClassNameString) => (
	<AriaMenuItem
		className={className}
		{...rest}
	/>
);
