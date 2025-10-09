import type { MinecraftCredentials } from '@/bindings.gen';
import { Button } from '@onelauncher/common/components';
import { Trash01Icon } from '@untitled-theme/icons-react';
import { DialogTrigger } from 'react-aria-components';
import { Overlay, RemoveAccountModal } from './overlay';

export function DeleteAccountButton({ profile, onPress }: { profile: MinecraftCredentials; onPress: () => void }) {
	return (
		<DialogTrigger>
			<Button className="group w-8 h-8" color="ghost" size="icon">
				<Trash01Icon className="group-hover:stroke-danger" />
			</Button>

			<Overlay>
				<RemoveAccountModal onPress={onPress} profile={profile} />
			</Overlay>
		</DialogTrigger>
	);
}
