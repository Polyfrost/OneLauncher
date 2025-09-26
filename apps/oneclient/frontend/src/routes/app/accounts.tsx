import type { MinecraftCredentials } from '@/bindings.gen';
import type { ButtonProps } from '@onelauncher/common/components';
import { AccountAvatar, SheetPage, SkinViewer } from '@/components';
import { AddAccountModal } from '@/components/overlay';
import { Overlay } from '@/components/overlay/Overlay';
import { RemoveAccountModal } from '@/components/overlay/RemoveAccountModal';
import { usePlayerProfile } from '@/hooks/usePlayerProfile';
import { bindings } from '@/main';
import { useCommandMut, useCommandSuspense } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { useQueryClient } from '@tanstack/react-query';
import { createFileRoute } from '@tanstack/react-router';
import { Trash01Icon } from '@untitled-theme/icons-react';
import { Button as AriaButton, DialogTrigger } from 'react-aria-components';
import { twMerge } from 'tailwind-merge';

export const Route = createFileRoute('/app/accounts')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<SheetPage
			headerLarge={<HeaderLarge />}
			headerSmall={<HeaderSmall />}
		>
			<SheetPage.Content>
				<div className="flex-1 flex flex-row gap-8">
					<Viewer />

					<div className="min-h-full w-px bg-component-border"></div>

					<AccountList />
				</div>
			</SheetPage.Content>
		</SheetPage>
	);
}

function HeaderLarge() {
	return (
		<div className="flex flex-row justify-between items-end gap-16">
			<div className="flex-1 flex flex-col">
				<h1 className="text-3xl font-semibold">Accounts</h1>
				<p className="text-md font-medium text-fg-secondary">Something something in corporate style fashion about picking your preferred gamemodes and versions and optionally loader so that oneclient can pick something for them</p>
			</div>

			<AddAccountButton size="large" />
		</div>
	);
}

function HeaderSmall() {
	return (
		<div className="flex flex-row justify-between items-center h-full">
			<h1 className="text-2lg h-full font-medium">Accounts</h1>

			<AddAccountButton size="normal" />
		</div>
	);
}

function Viewer() {
	const { data: currentAccount } = useCommandSuspense(['getDefaultUser'], () => bindings.core.getDefaultUser(true));
	const { data: profile } = usePlayerProfile(currentAccount?.id);

	return (
		<SkinViewer
			capeUrl={profile?.cape_url}
			className="h-full w-full max-w-1/4"
			height={400}
			skinUrl={profile?.skin_url}
			width={250}
		/>
	);
}

function AccountList() {
	const { data: accounts } = useCommandSuspense(['getUsers'], () => bindings.core.getUsers());
	const { data: currentAccount } = useCommandSuspense(['getDefaultUser'], () => bindings.core.getDefaultUser(true));

	const queryClient = useQueryClient();
	const { mutate: setDefaultUser } = useCommandMut(bindings.core.setDefaultUser, {
		onSuccess() {
			queryClient.invalidateQueries({
				queryKey: ['getDefaultUser'],
			});
		},
	});
	const { mutate: removeUser } = useCommandMut(bindings.core.removeUser, {
		onSuccess() {
			queryClient.invalidateQueries({
				queryKey: ['getDefaultUser'],
			});
			queryClient.invalidateQueries({
				queryKey: ['getUsers'],
			});
		},
	});

	return (
		<div className="flex-1 flex flex-col gap-2">
			{accounts.map(account => (
				<AccountRow
					key={account.id}
					onDelete={() => removeUser(account.id)}
					onPress={() => setDefaultUser(account.id)}
					profile={account}
					selected={currentAccount?.id === account.id}
				/>
			))}

			{accounts.length === 0 && (
				<p className="h-full flex justify-center items-center text-fg-secondary text-sm">No accounts found.</p>
			)}
		</div>
	);
}

function AccountRow({
	selected = false,
	profile,
	onPress,
	onDelete,
}: {
	selected?: boolean;
	profile: MinecraftCredentials;
	onPress: () => void;
	onDelete: () => void;
}) {
	const { isError } = usePlayerProfile(profile.id);

	return (
		<AriaButton
			className={twMerge(
				'flex flex-row items-center justify-start p-2 gap-2 bg-component-bg hover:bg-component-bg-hover pressed:bg-component-bg-pressed outline rounded-lg',
				selected
					? 'outline-brand hover:outline-brand-hover pressed:outline-brand-pressed'
					: 'outline-component-border hover:outline-component-border-hover pressed:outline-component-border-pressed',
			)}
			onPress={onPress}
		>
			<div className="flex flex-row justify-between w-full">
				<div className="flex flex-row gap-2">
					<AccountAvatar className="aspect-square h-12 rounded-sm " uuid={profile.id} />

					<div className="text-left flex flex-col">
						<p className="flex items-center gap-1 text-fg-primary font-semibold">
							{profile.username}
							{isError && <span className="text-danger text-xs font-medium"> (Error fetching online profile)</span>}
						</p>
						<p className="text-fg-secondary text-sm">{profile.id}</p>
					</div>
				</div>

				<div className="flex flex-row items-center gap-2">
					<DialogTrigger>
						<Button className="w-8 h-8" color="ghost" size="icon">
							<Trash01Icon />
						</Button>

						<Overlay>
							<RemoveAccountModal onPress={onDelete} profile={profile} />
						</Overlay>
					</DialogTrigger>
				</div>
			</div>
		</AriaButton>
	);
}

function AddAccountButton({
	size,
}: {
	size: ButtonProps['size'];
}) {
	return (
		<DialogTrigger>
			<Button color="secondary" size={size}>
				Add Account
			</Button>

			<Overlay>
				<AddAccountModal />
			</Overlay>
		</DialogTrigger>
	);
}
