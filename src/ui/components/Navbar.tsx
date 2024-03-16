import { A } from '@solidjs/router';

type NavbarButtonProps = {
    path: string,
    label: string,
};

function NavbarButton(props: NavbarButtonProps) {
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
    return (
        <div class='flex flex-row justify-evenly items-center h-15'>
            <div class='flex flex-row gap-x-10 py-1'>
                <NavbarButton path="/" label="Home" />
                <NavbarButton path="/browser" label="Browser" />
                <NavbarButton path="/updates" label="Updates" />
            </div>
        </div>
    );
}

export default Navbar;
