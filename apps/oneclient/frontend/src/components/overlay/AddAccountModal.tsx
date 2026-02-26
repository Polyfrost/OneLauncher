import type { MinecraftCredentials } from '@/bindings.gen';
import { AccountAvatar, isMinecraftAuthError, MinecraftAuthErrorModal, Overlay } from '@/components';
import { bindings } from '@/main';
import { useCommandMut } from '@onelauncher/common';
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';

export function AddAccountModal() {
	return (
		<Overlay.Dialog isDismissable>
			<Overlay.Title>Add Account</Overlay.Title>
			<p className="text-fg-secondary">Add a new account to OneClient</p>
			<AddAccountModalButton />
		</Overlay.Dialog>
	);
}

export function AddAccountModalButton() {
	const queryClient = useQueryClient();
	const [authError, setAuthError] = useState<unknown>(null);

	const { data: profile, isPending, mutate: login } = useCommandMut(bindings.core.openMsaLogin, {
		onSuccess(data) {
			setAuthError(null);
			queryClient.invalidateQueries({
				queryKey: ['getUsers'],
			});
			queryClient.invalidateQueries({
				queryKey: ['getDefaultUser'],
			});
		},
		onError(error) {
			console.error('[auth] AddAccountModal: openMsaLogin failed', error);
			if (isMinecraftAuthError(error))
				setAuthError(error);
		},
	});

	const onClick = () => {
		setAuthError(null);
		login();
	};

	if (authError)
		return <MinecraftAuthErrorModal error={authError} />;

	return (
		profile
			? (
					<>
						<AccountRow profile={profile} />
						<Overlay.Buttons buttons={[{ color: 'primary', children: 'Close', slot: 'close' }]} />
					</>
				)
			: <Overlay.Buttons buttons={[{ color: 'primary', children: 'Add Account', isPending, onClick }]} />
	);
}

function AccountRow({ profile }: { profile: MinecraftCredentials }) {
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
