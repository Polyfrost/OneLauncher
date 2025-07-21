import type { RefObject } from 'react';
import type { MenuItemProps, MenuProps, SeparatorProps } from 'react-aria-components';
import { useEffect, useRef, useState } from 'react';
import { Menu, MenuItem, Popover, Separator } from 'react-aria-components';
import { twMerge } from 'tailwind-merge';

export interface ContextMenuProps<T extends object> extends MenuProps<T> {
	className?: string;
	isOpen: boolean;
	setOpen: (open: boolean) => void;
	triggerRef: RefObject<HTMLElement | null>;
}

export function ContextMenu<T extends object>({
	className,
	isOpen,
	setOpen,
	triggerRef,
	children,
	...rest
}: ContextMenuProps<T>) {
	const [position, setPosition] = useState({ x: 0, y: 0 });
	const menuRef = useRef<HTMLDivElement>(null);

	useEffect(() => {
		const trigger = triggerRef.current;
		if (!trigger)
			return;

		const onContextMenu = (e: MouseEvent) => {
			e.preventDefault();
			e.stopPropagation();

			if (isOpen) {
				setOpen(false);
				return;
			}

			setPosition({
				x: e.clientX,
				y: e.clientY,
			});
			setOpen(true);
		};

		trigger.addEventListener('contextmenu', onContextMenu);

		return () => {
			trigger.removeEventListener('contextmenu', onContextMenu);
		};
	}, [isOpen, setOpen, triggerRef]);

	useEffect(() => {
		const onClick = (e: MouseEvent) => {
			if (e.target && menuRef.current?.contains(e.target as Node))
				return;

			setOpen(false);
		};

		window.addEventListener('click', onClick);

		return () => {
			window.removeEventListener('click', onClick);
		};
	}, [setOpen, triggerRef]);

	return (
		<Popover
			isNonModal
			isOpen={isOpen}
			onOpenChange={setOpen}
			shouldFlip
			triggerRef={triggerRef}
		>
			<Menu
				aria-label="contextmenu"
				autoFocus
				className={twMerge('rounded-lg bg-page-elevated border border-component-border p-2', className)}
				ref={menuRef}
				style={{ left: position.x, top: position.y }}
				{...rest}
			>
				{children}
			</Menu>
		</Popover>
	);
}

ContextMenu.Item = <T extends object>({ className, ...rest }: MenuItemProps<T>) => <MenuItem {...rest} className={twMerge('rounded-sm px-3 py-1 hover:bg-component-bg-hover', className?.toString())} />;

ContextMenu.Separator = ({ className, ...rest }: SeparatorProps) => <Separator {...rest} className={twMerge('my-1 py-0.25 bg-component-border', className)} />;
