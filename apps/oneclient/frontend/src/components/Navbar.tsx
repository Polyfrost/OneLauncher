import type { ButtonProps } from '@onelauncher/common/components';
import type { LinkProps, RegisteredRouter } from '@tanstack/react-router';
import LauncherLogo from '@/assets/logos/oneclient.svg?react';
import { AccountAvatar, AccountPopup, Overlay, SuperSecretDevOptions } from '@/components';
import { bindings } from '@/main';
import { useCommand } from '@onelauncher/common';
import { Button, Popup } from '@onelauncher/common/components';
import { Link, useNavigate } from '@tanstack/react-router';
import { Window } from '@tauri-apps/api/window';
import { MinusIcon, Settings02Icon, SquareIcon, XCloseIcon } from '@untitled-theme/icons-react';
import { twMerge } from 'tailwind-merge';

export function Navbar() {
	const { data: currentAccount } = useCommand(['getDefaultUser'], () => bindings.core.getDefaultUser(true));
	const navigate = useNavigate();

	const onMinimize = () => Window.getCurrent().minimize();
	const onMaximize = () => Window.getCurrent().toggleMaximize();
	const onClose = () => Window.getCurrent().close();

	return (
		<nav className="flex flex-row items-center justify-between h-20 px-12 z-40" data-tauri-drag-region="true">
			<div className="flex flex-1">
				<Overlay.Trigger>
					<NavbarButton className="w-[230px] h-[47px] focus:outline-none focus:ring-0" color="none">
						<LauncherLogo height={47} width={230} />
					</NavbarButton>

					<Overlay>
						<SuperSecretDevOptions />
					</Overlay>
				</Overlay.Trigger>
			</div>

			<div className="flex flex-1 items-center justify-center pointer-events-none gap-6">
				<NavbarLink to=".">Home</NavbarLink>
				<NavbarLink to="./clusters">Versions</NavbarLink>
				<NavbarLink to="./accounts">Accounts</NavbarLink>
			</div>

			<div className="flex flex-1 items-center justify-end gap-6 pointer-events-none">
				<div className="flex gap-4">
					<NavbarButton
						children={<Settings02Icon height={28} width={28} />}
						onClick={() => navigate({ to: '/app/settings' })}
					/>
					<Popup.Trigger>
						<NavbarButton>
							<AccountAvatar className="w-full h-full rounded-lg" uuid={currentAccount?.id} />
						</NavbarButton>
						<AccountPopup />
					</Popup.Trigger>
				</div>

				<div className="flex gap-2">
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
			</div>

		</nav>
	);
}

function NavbarLink({
	to,
	children,
}: LinkProps<'a', RegisteredRouter, '/app'>) {
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
				from="/app"
				inactiveProps={{
					className: 'text-fg-secondary font-normal partial-underline-0% hover:partial-underline-60% hover:text-fg-secondary-hover after:text-fg-secondary-hover pointer-events-auto',
				}}
				to={to}
			>
				{children}
			</Link>
		</div>
	);
}

export function NavbarButton({
	children,
	color = 'ghost',
	size = 'iconLarge',
	onClick,
	className = '',
	...rest
}: ButtonProps) {
	return (
		<Button
			className={twMerge('flex items-center justify-center w-10 h-10 pointer-events-auto', className)}
			color={color}
			onClick={onClick}
			size={size}
			{...rest}
		>
			{children}
		</Button>
	);
}
