import { WEBSITE } from '@onelauncher/client';
import { A, useNavigate } from '@solidjs/router';
import { open } from '@tauri-apps/plugin-shell';
import { Bell01Icon, Cloud01Icon, Settings01Icon, TerminalBrowserIcon } from '@untitled-theme/icons-solid';
import { createSignal } from 'solid-js';
import Button from './base/Button';
import PlayerHead from './game/PlayerHead';
import PolyfrostFull from './logos/PolyfrostFull';
import AccountPopup from './overlay/account/AccountsPopup';
import useAccountController from './overlay/account/AddAccountModal';
import NotificationPopup from './overlay/notifications/NotificationPopup';
import Popup from './overlay/Popup';

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

	let profileButton!: HTMLButtonElement;
	let notificationButton!: HTMLButtonElement;

	return (
		<div class="h-15 min-h-[60px] flex flex-row items-center *:flex-1">
			<div>
				<div class="w-min flex items-center justify-center transition-transform active:scale-90" onClick={() => open(WEBSITE)}>
					<PolyfrostFull />
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
				<Button
					buttonStyle="icon"
					class="relative [&>div]:absolute"
					onClick={() => setNotificationMenuOpen(!notificationMenuOpen())}
					ref={notificationButton}
				>
					<Bell01Icon />
				</Button>

				{/* Launcher Settings Button */}
				<Button buttonStyle="icon" onClick={() => navigate('/settings')}>
					<Settings01Icon />
				</Button>

				{/* Account Menu Button */}
				<button
					class="hover:opacity-70"
					onClick={() => setProfileMenuOpen(!profileMenuOpen())}
					ref={profileButton}
				>
					<PlayerHead class="h-8 max-h-8 max-w-8 min-h-8 min-w-8 w-8 rounded-md" uuid={controller.defaultAccount()?.id} />
				</button>
			</div>

			<AccountPopup
				class="mt-2"
				ref={el => Popup.setPos(profileButton, el)}
				setVisible={setProfileMenuOpen}
				visible={profileMenuOpen}
			/>

			<NotificationPopup
				class="mt-2"
				ref={el => Popup.setPos(notificationButton, el)}
				setVisible={setNotificationMenuOpen}
				visible={notificationMenuOpen}
			/>
		</div>
	);
}

export default Navbar;
