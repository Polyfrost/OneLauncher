import type { ToOptions } from '@tanstack/react-router';
import useCommand from '@/hooks/useCommand';
import { bindings } from '@/main';
import { Link } from '@tanstack/react-router';
import { Bell01Icon, Cloud01Icon, TerminalBrowserIcon } from '@untitled-theme/icons-react';
import { MenuTrigger } from 'react-aria-components';
import Button from './base/Button';
import Menu from './base/Menu';
import PlayerHead from './content/PlayerHead';
import AccountPopup from './overlay/popups/AccountPopup';

function Navbar() {
	const defaultUser = useCommand('getDefaultUser', () => bindings.core.get_default_user(false));

	return (
		<div className="h-15 min-h-15 flex flex-row items-center *:flex-1">
			<div className="flex items-start justify-start">
				<div className="flex items-start justify-start transition-transform active:scale-90">
					{/* <OneLauncherText width={260} /> */}
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
						<Button color="ghost" size="icon">
							<Bell01Icon />
						</Button>

						<Menu.Popover placement="bottom right">
							<Menu>
								<Menu.Item>Hello World</Menu.Item>
							</Menu>
						</Menu.Popover>
					</MenuTrigger>

					{/* <NotificationPopup
						className="mt-2"
						mount={notificationButtonContainer}
						ref={el => el.classList.add('right-0')}
						setVisible={setNotificationMenuOpen}
						visible={notificationMenuOpen}
					/> */}
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
