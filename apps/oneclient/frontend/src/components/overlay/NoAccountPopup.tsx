import { AddAccountModal, Overlay } from '@/components/overlay';
import { Button } from '@onelauncher/common/components';

export function NoAccountPopup() {
	return (
		<Overlay.Dialog>
			<Overlay.Title>No Account</Overlay.Title>
			<p className="max-w-sm text-fg-secondary">Please add an account before you start minecraft</p>

			<Overlay.Trigger>
				<Button className="w-full" size="large">
					Add Account
				</Button>

				<Overlay>
					<AddAccountModal />
				</Overlay>
			</Overlay.Trigger>
		</Overlay.Dialog>
	);
}
