import { WEBSITE } from '@onelauncher/client';
import { A, useNavigate } from '@solidjs/router';
import { open } from '@tauri-apps/plugin-shell';
import { Bell01Icon, Cloud01Icon, Settings01Icon, TerminalBrowserIcon } from '@untitled-theme/icons-solid';
import OneLauncherText from '~assets/logos/onelauncher_text.svg?component-solid';
import { createSignal } from 'solid-js';
import Button from './base/Button';
import PlayerHead from './game/PlayerHead';
import AccountPopup from './overlay/account/AccountsPopup';
import useAccountController from './overlay/account/AddAccountModal';
import NotificationPopup from './overlay/notifications/NotificationPopup';

interface NavbarLinkProps {
	path: string;
	label: string;
}

function NavbarLink(props: NavbarLinkProps) {
	return (
		<A
			activeClass="text-fg-primary"
			class="rounded-lg px-4 py-2 text-lg hover:bg-component-bg-hover hover:text-fg-primary-hover"
			end={props.path === '/'}
			href={props.path}
			inactiveClass="text-fg-secondary"
		>
			{props.label}
		</A>
	);
}

function Navbar() {
	const [profileMenuOpen, setProfileMenuOpen] = createSignal(false);
	const [notificationMenuOpen, setNotificationMenuOpen] = createSignal(false);
	const controller = useAccountController();

	const navigate = useNavigate();

	let profileButtonContainer!: HTMLDivElement;
	let notificationButtonContainer!: HTMLDivElement;

	return (
		<div class="h-15 min-h-15 flex flex-row items-center *:flex-1">
			<div class="flex items-start justify-start">
				<div class="flex items-start justify-start transition-transform active:scale-90" onClick={() => open(WEBSITE)}>
					<OneLauncherText width={260} />
				</div>
			</div>
			<div class="flex flex-row items-center justify-center gap-x-10 py-1">
				<NavbarLink label="Home" path="/" />
				<NavbarLink label="Browser" path="/browser" />
				<NavbarLink label="Updates" path="/updates" />
			</div>
			<div class="relative flex flex-row items-center justify-end gap-x-2">
				<Button buttonStyle="icon">
					<TerminalBrowserIcon />
				</Button>

				<Button buttonStyle="icon">
					<Cloud01Icon />
				</Button>

				{/* Notification Manager Button */}
				<div class="relative" ref={notificationButtonContainer}>
					<Button
						buttonStyle="icon"
						class="relative [&>div]:absolute"
						onClick={() => setNotificationMenuOpen(!notificationMenuOpen())}
					>
						<Bell01Icon />
					</Button>

					<NotificationPopup
						class="mt-2"
						mount={notificationButtonContainer}
						ref={el => el.classList.add('right-0')}
						setVisible={setNotificationMenuOpen}
						visible={notificationMenuOpen}
					/>
				</div>

				{/* Launcher Settings Button */}
				<Button buttonStyle="icon" onClick={() => navigate('/settings')}>
					<Settings01Icon />
				</Button>

				{/* Account Menu Button */}
				<div class="relative" ref={profileButtonContainer}>
					<button
						class="hover:opacity-70"
						onClick={() => setProfileMenuOpen(!profileMenuOpen())}
					>
						<PlayerHead class="h-8 max-h-8 max-w-8 min-h-8 min-w-8 w-8 rounded-md" uuid={controller.defaultAccount()?.id} />
					</button>

					<AccountPopup
						class="mt-2"
						mount={profileButtonContainer}
						ref={el => el.classList.add('right-0')}
						setVisible={setProfileMenuOpen}
						visible={profileMenuOpen}
					/>
				</div>
			</div>
		</div>
	);
}

export default Navbar;
