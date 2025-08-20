import type { MinecraftCredentials } from '@/bindings.gen';
import { AccountAvatar } from '@/components/AccountAvatar';
import { bindings } from '@/main';
import { useCommandMut, useCommandSuspense } from '@onelauncher/common';
import { useQueryClient } from '@tanstack/react-query';
import { Button as AriaButton } from 'react-aria-components';
import { twMerge } from 'tailwind-merge';
import { Popup } from './Popup';

export function AccountPopup() {
	const { data: currentAccount } = useCommandSuspense(['getDefaultUser'], () => bindings.core.getDefaultUser(true));
	const { data: accounts } = useCommandSuspense(['getUsers', 'accountPopup'], bindings.core.getUsers, {
		select(data) {
			data.sort((a, b) => {
				if (currentAccount?.id === a.id)
					return -1;
				else if (currentAccount?.id === b.id)
					return 1;
				return a.username.localeCompare(b.username);
			});

			return data;
		},
	});

	const queryClient = useQueryClient();
	const { mutate: setDefaultUser } = useCommandMut(bindings.core.setDefaultUser, {
		onSuccess() {
			queryClient.invalidateQueries({
				queryKey: ['getDefaultUser'],
			});
		},
	});

	return (
		<Popup placement="top left">
			<div className="flex flex-col gap-1 min-w-40">
				{accounts.map(profile => (
					<AccountRow
						key={profile.id}
						onPress={() => setDefaultUser(profile.id)}
						profile={profile}
						selected={profile.id === currentAccount?.id}
					/>
				))}
			</div>
		</Popup>
	);
}

function AccountRow({
	selected = false,
	profile,
	onPress,
}: {
	selected?: boolean;
	profile: MinecraftCredentials;
	onPress: () => void;
}) {
	return (
		<AriaButton
			className={twMerge(
				'flex flex-row items-center justify-start p-1 gap-2 hover:bg-component-bg-hover pressed:bg-component-bg-pressed rounded-lg',
				selected && 'bg-brand/80 pointer-events-none',
			)}
			onPress={onPress}
			slot="close"
		>
			<AccountAvatar className="aspect-square h-8 rounded-md" uuid={profile.id} />

			<div className="text-left flex flex-col">
				<p className="flex items-center gap-1 text-fg-primary font-medium">{profile.username}</p>
			</div>
		</AriaButton>
	);
}
