import type { ButtonProps } from '@onelauncher/common/components';
import type { LinkProps } from '@tanstack/react-router';
import LauncherLogo from '@/assets/logos/oneclient.svg?react';
import { Button } from '@onelauncher/common/components';
import { Link } from '@tanstack/react-router';
import { Window } from '@tauri-apps/api/window';
import { MinusIcon, SquareIcon, XCloseIcon } from '@untitled-theme/icons-react';
import { twMerge } from 'tailwind-merge';

export function Navbar() {
	const onMinimize = () => Window.getCurrent().minimize();
	const onMaximize = () => Window.getCurrent().toggleMaximize();
	const onClose = () => Window.getCurrent().close();

	return (
		<nav className="flex flex-row items-center justify-between h-20" data-tauri-drag-region="true">
			<div className="flex flex-1 pointer-events-none">
				<LauncherLogo height={47} width={230} />
			</div>

			<div className="flex flex-1 items-center justify-center pointer-events-none gap-6">
				<NavbarLink to="/app">Home</NavbarLink>
				<NavbarLink to="/app/test">Clusters</NavbarLink>
				<NavbarLink to="/">Accounts</NavbarLink>
			</div>

			<div className="flex flex-1 items-center justify-end gap-2">
				<NavbarButton
					children={<MinusIcon />}
					onClick={onMinimize}
				/>
				<NavbarButton
					children={<SquareIcon />}
					onClick={onMaximize}
				/>
				<NavbarButton
					children={(
						<XCloseIcon
							height={28}
							strokeWidth={1.5}
							width={28}
						/>
					)}
					className="bg-transparent"
					color="danger"
					onClick={onClose}
				/>
			</div>

		</nav>
	);
}

function NavbarLink({
	to,
	children,
}: LinkProps) {
	return (
		<div className="flex-1 flex justify-center items-center">
			<Link
				activeOptions={{
					exact: true,
				}}
				activeProps={{
					className: 'text-fg-primary font-semibold partial-underline-75% pointer-events-none',
				}}
				className="text-center text-2lg transition-all duration-100 after:duration-100 after:transition-all"
				inactiveProps={{
					className: 'text-fg-secondary font-normal partial-underline-0% hover:partial-underline-50% hover:font-medium hover:text-fg-secondary-hover after:text-fg-secondary-hover pointer-events-auto',
				}}
				to={to}
			>
				{children}
			</Link>
		</div>
	);
}

function NavbarButton({
	children,
	color = 'ghost',
	size = 'iconLarge',
	onClick,
	className = '',
	...rest
}: ButtonProps) {
	return (
		<Button
			className={twMerge('flex items-center justify-center w-12 h-12 pointer-events-auto', className)}
			color={color}
			onClick={onClick}
			size={size}
			{...rest}
		>
			{children}
		</Button>
	);
}
