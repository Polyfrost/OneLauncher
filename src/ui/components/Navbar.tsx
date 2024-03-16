import { A } from '@solidjs/router';
import { ParentProps, createSignal } from 'solid-js';
import PolyfrostFull from './logos/PolyfrostFull';
import AccountPopup from './overlay/AccountPopup';

type NavbarLinkProps = {
    path: string,
    label: string,
};

function NavbarLink(props: NavbarLinkProps) {
    return (
        <A
            href={props.path}
            class='text-lg px-4 py-1 hover:bg-component-bg-hover rounded-lg hover:text-fg-primary-hover'
            inactiveClass='text-fg-secondary'
            activeClass='text-fg-primary'
            end
        >
            {props.label}
        </A>
    );
}

function Navbar() {
    const [profileMenuOpen, setProfileMenuOpen] = createSignal(false);
    let profileButton!: HTMLButtonElement;

    return (
        <div class='flex flex-row *:flex-1 items-center h-15 mx-2'>
            <div>
                <PolyfrostFull />
            </div>
            <div class='flex flex-row items-center justify-center gap-x-10 py-1'>
                <NavbarLink path="/" label="Home" />
                <NavbarLink path="/browser" label="Browser" />
                <NavbarLink path="/updates" label="Updates" />
            </div>
            <div class='flex flex-row justify-end relative'>
                <button ref={profileButton} onClick={() => setProfileMenuOpen(!profileMenuOpen())}>
                    <img class='w-7 h-7 rounded-md hover:opacity-70' src="https://crafatar.com/avatars/f247be7c5b8241c69148793ded77e71f?size=100" alt="" />
                </button>
            </div>

            <AccountPopup
                visible={profileMenuOpen}
                setVisible={setProfileMenuOpen}
                mount={profileButton}
                class='right-0 top-full mt-2'
            />
        </div>
    );
}

export default Navbar;
