import { AddAccountModalButton, Overlay } from '@/components';

export function NoAccountPopup() {
	return (
		<Overlay.Dialog>
			<Overlay.Title>No Account</Overlay.Title>
			<p className="max-w-sm text-fg-secondary">Please add an account before you start minecraft</p>
			<AddAccountModalButton />
		</Overlay.Dialog>
	);
}
