import type { MinecraftCredentials } from '@/bindings.gen';
import { Overlay, RemoveAccountModal } from '@/components';
import { Button } from '@onelauncher/common/components';
import { Trash01Icon } from '@untitled-theme/icons-react';

export function DeleteAccountButton({ profile, onPress }: { profile: MinecraftCredentials; onPress: () => void }) {
	return (
		<Overlay.Trigger>
			<Button className="group w-8 h-8" color="ghost" size="icon">
				<Trash01Icon className="group-hover:stroke-danger" />
			</Button>

			<Overlay>
				<RemoveAccountModal onPress={onPress} profile={profile} />
			</Overlay>
		</Overlay.Trigger>
	);
}
