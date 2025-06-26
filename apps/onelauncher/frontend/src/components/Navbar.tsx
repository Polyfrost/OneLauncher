import type { ToOptions } from '@tanstack/react-router';
import { bindings } from '@/main';
import useNotifications from '@/hooks/useNotification';
import { useCommand } from '@onelauncher/common';
import { Button, Menu } from '@onelauncher/common/components';
import { Link } from '@tanstack/react-router';
import { Bell01Icon, Cloud01Icon, TerminalBrowserIcon } from '@untitled-theme/icons-react';
import { MenuTrigger } from 'react-aria-components';
import OneLauncherText from '../assets/logos/onelauncher_text.svg';
import PlayerHead from './content/PlayerHead';
import AccountPopup from './overlay/popups/AccountPopup';
import NotificationPopup from './overlay/popups/NotificationPopup';

function Navbar() {
	const defaultUser = useCommand('getDefaultUser', () => bindings.core.getDefaultUser(false));
	const { list } = useNotifications();
	const notificationCount = Object.keys(list).length;

	return (
		<div className="h-15 min-h-15 flex flex-row items-center *:flex-1">
			<div className="flex items-start justify-start">
				<div className="flex items-start justify-start transition-transform active:scale-90">
					{/* <OneLauncherText width={260} /> */}
					<img src={OneLauncherText} width={260} />
				</div>
			</div>
			<div className="flex flex-row items-center gap-x-10 py-1">
				<NavbarLink label="Home" path="/app" />
				<NavbarLink label="Browser" path="/app/browser" />
				<NavbarLink label="Settings" path="/app/settings" />
			</div>
			<div className="relative flex flex-row items-center justify-end gap-x-2">
				<Button color="ghost" size="icon">
					<TerminalBrowserIcon />
				</Button>

				<Button color="ghost" size="icon">
					<Cloud01Icon />
				</Button>

				{/* Notification Manager Button */}
				<div className="relative">
					<MenuTrigger>
						<Button className="relative" color="ghost" size="icon">
							<Bell01Icon />
							{notificationCount > 0 && (
								<span className="absolute -top-1 -right-1 bg-red-500 text-white text-xs rounded-full h-5 w-5 flex items-center justify-center font-medium">
									{notificationCount > 99 ? '99+' : notificationCount}
								</span>
							)}
						</Button>

						<Menu.Popover placement="bottom right">
							<NotificationPopup />
						</Menu.Popover>
					</MenuTrigger>
				</div>

				<MenuTrigger>
					<Button className="p-0" color="ghost" size="icon">
						<PlayerHead className="h-full rounded-md hover:opacity-70" uuid={defaultUser.data?.id} />
					</Button>

					<Menu.Popover>
						<AccountPopup />
					</Menu.Popover>
				</MenuTrigger>
			</div>
		</div>
	);
}

export default Navbar;

function NavbarLink({
	path,
	label,
}: {
	path: ToOptions['to'];
	label: string;
}) {
	return (
		<Link
			activeOptions={{
				exact: true,
			}}
			activeProps={{
				className: 'text-fg-primary',
			}}
			className="flex-1 text-center rounded-lg px-4 py-1 text-lg hover:bg-component-bg-hover hover:text-fg-primary-hover"
			inactiveProps={{
				className: 'text-fg-secondary',
			}}
			to={path}
		>
			{label}
		</Link>
	);
}
