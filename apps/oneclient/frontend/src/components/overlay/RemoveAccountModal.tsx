import type { MinecraftCredentials } from '@/bindings.gen';
import { AccountAvatar } from '@/components/AccountAvatar';
import { Button } from '@onelauncher/common/components';
import { Overlay } from './Overlay';

export function RemoveAccountModal({
	profile,
	onPress,
}: {
	profile: MinecraftCredentials;
	onPress: () => void;
}) {
	return (
		<Overlay.Dialog>
			<Overlay.Title>Are you sure?</Overlay.Title>

			<p>
				Do you want to remove
				<span className="font-bold" color="primary">{profile.username}</span>
				{' '}
				from you're accounts?
			</p>
			<p>This cannot be undone</p>

			<AccountRow profile={profile} />
			<Button
				className="w-full"
				color="danger"
				onPress={onPress}
				size="large"
				slot="close"
			>
				Remove
			</Button>
		</Overlay.Dialog>
	);
}

function AccountRow({
	profile,
}: {
	profile: MinecraftCredentials;
}) {
	return (
		<div className="flex flex-row items-center justify-start gap-2">
			<AccountAvatar className="aspect-square h-12 rounded-sm " uuid={profile.id} />

			<div className="text-left flex flex-col">
				<p className="flex items-center gap-1 text-fg-primary font-semibold">{profile.username}</p>
				<p className="text-fg-secondary text-sm">{profile.id}</p>
			</div>
		</div>
	);
}
