import { Button } from '@onelauncher/common/components';
import { DialogTrigger } from 'react-aria-components';
import { AddAccountModal } from './AddAccountModal';
import { Overlay } from './Overlay';

export function NoAccountPopup() {
	return (
		<Overlay.Dialog>
			<Overlay.Title>No Account</Overlay.Title>
			<p>Please add an account before you start minecraft</p>

			<DialogTrigger>
				<Button className="w-full" size="large">
					Add Account
				</Button>

				<Overlay>
					<AddAccountModal />
				</Overlay>
			</DialogTrigger>
		</Overlay.Dialog>
	);
}
