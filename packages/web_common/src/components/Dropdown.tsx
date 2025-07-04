import type {
	ListBoxItemProps as AriaListBoxItemProps,
	SelectProps as AriaSelectProps,

} from 'react-aria-components';
import type { VariantProps } from 'tailwind-variants';
import type { ClassNameString } from '../types/global';
import { ChevronDownIcon, ChevronUpIcon } from '@untitled-theme/icons-react';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import React from 'react';
import {
	Label as AriaLabel,
	ListBox as AriaListBox,
	ListBoxItem as AriaListBoxItem,
	Popover as AriaPopover,
	Select as AriaSelect,
	SelectValue as AriaSelectValue,
} from 'react-aria-components';
import { twMerge } from 'tailwind-merge';
import { tv } from 'tailwind-variants';
import { Button } from './Button';

const dropdownStyles = tv({
	slots: {
		root: 'relative h-full data-[disabled]:opacity-10 data-[disabled]:pointer-events-none',

		trigger: [
			'h-full w-full px-3 py-1.5 rounded-lg flex items-center justify-between text-left',
			'border border-component-border',
			'bg-component-bg',
		],

		triggerValueContent: 'flex-1 h-full flex flex-row items-center gap-1 overflow-hidden text-nowrap',

		triggerChevronIcon: 'w-4 h-4 ml-2 flex-shrink-0 transition-transform data-[open]:rotate-180',

		triggerCustomIcon: 'w-4 h-4',

		popover: [
			// i'll fix this if i dont forget
			'mt-1 w-full rounded-lg shadow-md',
			'bg-component-bg border border-component-border',
			'data-[entering]:animate-in data-[entering]:fade-in data-[entering]:zoom-in-95',
			'data-[exiting]:animate-out data-[exiting]:fade-out data-[exiting]:zoom-out-95',
			'react-aria-Popover',
		],

		popoverContentWrapper: 'flex flex-col gap-1',

		listBox: 'outline-none flex flex-col gap-0.5',

		overlayScrollbars: 'max-h-46 overflow-auto',

		listToolRowWrapper: 'p-1 mt-1',
	},
	variants: {
		minimal: {
			true: {
				root: 'w-auto h-auto',
				trigger: [
					'p-1.5',
					'bg-transparent hover:bg-dropdown-minimal-trigger-bg-hover border-transparent',
				],
				triggerValueContent: 'hidden',
				popover: 'w-max',
			},
		},
	},
	defaultVariants: {
		minimal: false,
	},
});

const dropdownItemStyles = tv({
	slots: {
		container: [
			'group/item flex flex-row items-center justify-between gap-2 rounded-lg p-2 w-full',
			'text-dropdown-item-text hover:bg-component-bg-hover data-[focused]:bg-component-bg-pressed',
			'outline-none hover:cursor-pointer',
		],

		selectedIndicator: [
			'w-1.5 h-1.5 rounded-full flex-shrink-0 opacity-0 transition-opacity',
			'group-data-[selected]/item:opacity-100',
		],

		contentRowLayout: 'w-full flex flex-row justify-start items-center gap-1.5 h-5 text-nowrap overflow-hidden',
	},
});

export type DropdownStyleProps = VariantProps<typeof dropdownStyles>;

export interface DropdownProps<T extends object>
	extends Omit<AriaSelectProps<T>, 'className' | 'children'>,
	DropdownStyleProps,
	ClassNameString {
	label?: string;
	labelClassName?: string;
	children: React.ReactNode;
	listToolRow?: React.ReactNode;
	triggerTextPrefix?: string;

	customMinimalIcon?: React.ReactNode;
	popoverClassName?: string;
}

export function Dropdown<TValue extends object>(props: DropdownProps<TValue>) {
	const {
		className,
		label,
		labelClassName,
		children,
		listToolRow,
		triggerTextPrefix,
		customMinimalIcon,
		minimal,
		popoverClassName,
		...rest
	} = props;

	const styles = dropdownStyles({ minimal, className });

	return (
		<AriaSelect {...rest} className={styles.root()}>
			{({ isOpen }) => (
				<>
					{label && <AriaLabel className={labelClassName}>{label}</AriaLabel>}

					<Button className={styles.trigger()} color="ghost">
						<div className={styles.triggerValueContent()}>
							{triggerTextPrefix && <span>{triggerTextPrefix}</span>}
							<AriaSelectValue />
						</div>
						{
							isOpen
								? <ChevronUpIcon className={styles.triggerChevronIcon()} data-open />
								: <ChevronDownIcon className={styles.triggerChevronIcon()} />
						}
					</Button>

					<AriaPopover className={twMerge(styles.popover(), popoverClassName)}>
						<div className={styles.popoverContentWrapper()}>
							<OverlayScrollbarsComponent className={styles.overlayScrollbars()}>
								<AriaListBox className={styles.listBox()}>
									{children}
								</AriaListBox>
							</OverlayScrollbarsComponent>
							{listToolRow && (
								<div className={styles.listToolRowWrapper()}>
									{listToolRow}
								</div>
							)}
						</div>
					</AriaPopover>
				</>
			)}
		</AriaSelect>
	);
}

export interface DropdownItemProps extends AriaListBoxItemProps {
}

Dropdown.Item = (props: DropdownItemProps) => {
	const itemStyles = dropdownItemStyles();

	const { className, children } = props;

	return (
		<AriaListBoxItem {...props} className={twMerge(itemStyles.container(), className?.toString())}>
			{children}
		</AriaListBoxItem>
	);
};

export default Dropdown;
