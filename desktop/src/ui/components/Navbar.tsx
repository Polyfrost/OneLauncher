import { A } from '@solidjs/router';
import { createSignal } from 'solid-js';
import { Bell01Icon, Cloud01Icon, Settings01Icon, TerminalBrowserIcon } from '@untitled-theme/icons-solid';
import useAccount from '../hooks/useAccount';
import PolyfrostFull from './logos/PolyfrostFull';
import AccountPopup from './overlay/AccountPopup';
import PlayerHead from './game/PlayerHead';
import Button from './base/Button';

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
	const account = useAccount();

	let profileButton!: HTMLButtonElement;

	return (
		<div class="flex flex-row justify-center items-center min-h-[60px] h-15 mx-2">
			<div>
				<PolyfrostFull />
			</div>
			<div class="flex flex-1 flex-row items-center justify-center gap-x-10 py-1">
				<NavbarLink path="/" label="Home" />
				<NavbarLink path="/browser" label="Browser" />
				<NavbarLink path="/updates" label="Updates" />
			</div>
			<div class="flex flex-row justify-end items-center gap-x-2 relative">
				<Button styleType="icon" class="w-8 h-8">
					<TerminalBrowserIcon class="w-7 h-7" />
				</Button>

				<Button styleType="icon" class="w-8 h-8">
					<Cloud01Icon class="w-7 h-7" />
				</Button>

				<Button styleType="icon" class="w-8 h-8">
					<Bell01Icon class="w-7 h-7" />
				</Button>

				<Button styleType="icon" class="w-8 h-8">
					<Settings01Icon class="w-7 h-7" />
				</Button>

				<button ref={profileButton} onClick={() => setProfileMenuOpen(!profileMenuOpen())}>
					<PlayerHead class="w-[30px] h-[30px] rounded-md hover:opacity-70" uuid={account.uuid} />
				</button>
			</div>

			<AccountPopup
				visible={profileMenuOpen}
				setVisible={setProfileMenuOpen}
				mount={profileButton}
				class="right-0 top-full mt-2"
			/>
		</div>
	);
}

export default Navbar;
