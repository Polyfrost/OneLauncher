import { A, useNavigate } from '@solidjs/router';
import { createSignal } from 'solid-js';
import { Bell01Icon, Cloud01Icon, Settings01Icon, TerminalBrowserIcon } from '@untitled-theme/icons-solid';
import { open } from '@tauri-apps/plugin-shell';
import { WEBSITE } from '@onelauncher/client';
import PolyfrostFull from './logos/PolyfrostFull';
import AccountPopup from './overlay/account/AccountsPopup';
import PlayerHead from './game/PlayerHead';
import Button from './base/Button';
import NotificationPopup from './overlay/notifications/NotificationPopup';
import Popup from './overlay/Popup';
import useAccountController from './overlay/account/AddAccountModal';

interface NavbarLinkProps {
	path: string;
	label: string;
}

function NavbarLink(props: NavbarLinkProps) {
	return (
		<A
			href={props.path}
			class="text-lg px-4 py-2 hover:bg-component-bg-hover rounded-lg hover:text-fg-primary-hover"
			inactiveClass="text-fg-secondary"
			activeClass="text-fg-primary"
			end
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
		<div class="flex flex-row *:flex-1 items-center min-h-[60px] h-15">
			<div>
				<div onClick={() => open(WEBSITE)} class="flex items-center justify-center active:scale-90 transition-transform w-min">
					<PolyfrostFull />
				</div>
			</div>
			<div class="flex flex-row items-center justify-center gap-x-10 py-1">
				<NavbarLink path="/" label="Home" />
				<NavbarLink path="/browser" label="Browser" />
				<NavbarLink path="/updates" label="Updates" />
			</div>
			<div class="flex flex-row justify-end items-center gap-x-2 relative">
				<Button buttonStyle="icon">
					<TerminalBrowserIcon />
				</Button>

				<Button buttonStyle="icon">
					<Cloud01Icon />
				</Button>

				{/* Notification Manager Button */}
				<Button
					buttonStyle="icon"
					ref={notificationButton}
					class="[&>div]:absolute relative"
					onClick={() => setNotificationMenuOpen(!notificationMenuOpen())}
				>
					<Bell01Icon />
				</Button>

				{/* Launcher Settings Button */}
				<Button buttonStyle="icon" onClick={() => navigate('/settings')}>
					<Settings01Icon />
				</Button>

				{/* Account Menu Button */}
				<button
					ref={profileButton}
					onClick={() => setProfileMenuOpen(!profileMenuOpen())}
				>
					<PlayerHead class="w-[30px] h-[30px] rounded-md hover:opacity-70" uuid={controller.defaultAccount()?.id} />
				</button>
			</div>

			<AccountPopup
				visible={profileMenuOpen}
				setVisible={setProfileMenuOpen}
				ref={el => Popup.setPos(profileButton, el)}
				class="mt-2"
			/>

			<NotificationPopup
				visible={notificationMenuOpen}
				setVisible={setNotificationMenuOpen}
				ref={el => Popup.setPos(notificationButton, el)}
				class="mt-2"
			/>
		</div>
	);
}

export default Navbar;
