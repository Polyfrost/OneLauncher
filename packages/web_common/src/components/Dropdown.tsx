import type {
	ListBoxItemProps as AriaListBoxItemProps,
	SelectProps as AriaSelectProps,
} from 'react-aria-components';
import type { VariantProps } from 'tailwind-variants';
import type { ClassNameString } from '../types/global';
import {
	Label as AriaLabel,
	ListBox as AriaListBox,
	ListBoxItem as AriaListBoxItem,
	Popover as AriaPopover,
	Select as AriaSelect,
	SelectValue,
} from 'react-aria-components';
import { twMerge } from 'tailwind-merge';
import { tv } from 'tailwind-variants';
import { Button } from '.';

export interface ComboBoxWrapperProps<T extends object> extends Omit<AriaSelectProps<T>, 'className'>, ClassNameString {
	label?: string;
}

const dropdownVariants = tv({
	base: [
		'relative h-full',
	],
});

export type DropdownVariantsProps = VariantProps<typeof dropdownVariants>;

export function Dropdown<T extends object>({
	className,
	label,
	children,
	...rest
}: ComboBoxWrapperProps<T>) {
	return (
		<AriaSelect {...rest} className={dropdownVariants({ className })}>
			{label && <AriaLabel>{label}</AriaLabel>}
			<Button className="border w-full border-component-bg py-1" color="ghost">
				<SelectValue />
				<span aria-hidden="true">▼</span>
			</Button>
			<AriaPopover className="border border-component-bg rounded-lg p-1">
				<AriaListBox className="max-h-47 overflow-auto flex-col gap-1">
					{children}
				</AriaListBox>
			</AriaPopover>
		</AriaSelect>
	);
}

export default Dropdown;

Dropdown.Item = ({
	className,
	...rest
}: AriaListBoxItemProps & ClassNameString) => (
	<AriaListBoxItem
		className={twMerge('hover:bg-component-bg-hover px-2 py-1 rounded-md w-full', className)}
		{...rest}
	/>
);
