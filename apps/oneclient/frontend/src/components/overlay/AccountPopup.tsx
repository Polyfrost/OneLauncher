import type { MinecraftCredentials } from '@/bindings.gen';
import { bindings } from '@/main';
import { useCommand } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { Link } from '@tanstack/react-router';
import { PlusIcon, Settings01Icon } from '@untitled-theme/icons-react';
import { DialogTrigger } from 'react-aria-components';
import { twMerge } from 'tailwind-merge';
import { AccountAvatar } from '../AccountAvatar';
import { DeleteAccountButton } from '../DeleteAccountButton';
import { ManageSkinButton } from '../ManageSkinButton';
import { AddAccountModal } from './AddAccountModal';
import { Overlay } from './Overlay';
import { Popup } from './Popup';

export function AccountPopup() {
	const users = useCommand(['getUsers'], bindings.core.getUsers);
	const defaultUser = useCommand(['getDefaultUser'], () => bindings.core.getDefaultUser(false));

	const setDefaultUser = async (user: MinecraftCredentials) => {
		await bindings.core.setDefaultUser(user.id);
		users.refetch();
		defaultUser.refetch();
	};

	const deleteUser = async (user: MinecraftCredentials) => {
		await bindings.core.removeUser(user.id);
		users.refetch();
		defaultUser.refetch();

		if (defaultUser.data && defaultUser.data.id === user.id && users.data && users.data.length > 1) {
			const filtered = users.data.filter(userData => userData.id !== user.id);
			if (filtered.length > 0)
				setDefaultUser(filtered[0]);
		}
	};

	return (
		<Popup placement="top left">

			<div className="min-w-3xs">
				{defaultUser.data && (
					<div>
						<AccountEntry
							loggedIn
							onClick={() => { }}
							onDelete={() => deleteUser(defaultUser.data as MinecraftCredentials)}
							user={defaultUser.data}
						/>
					</div>
				)}

				{users.data?.filter(user => user.id !== defaultUser.data?.id).map(user => (
					<div key={user.id}>
						<AccountEntry
							onClick={() => setDefaultUser(user)}
							onDelete={() => deleteUser(user)}
							user={user}
						/>
					</div>
				))}

				<div className="flex flex-row justify-between">
					<div className="self-center">

						<DialogTrigger>
							<Button color="ghost">
								<PlusIcon />
								Add Account
							</Button>

							<Overlay>
								<AddAccountModal />
							</Overlay>
						</DialogTrigger>
					</div>
					<div className="flex flex-row">
						<Link to="/app/accounts">
							<Button color="ghost" size="iconLarge">
								<Settings01Icon className="stroke-fg-primary" />
							</Button>
						</Link>
					</div>
				</div>
			</div>
		</Popup>
	);
}

export default AccountPopup;

function AccountEntry({
	onClick,
	onDelete,
	user,
	loggedIn = false,
}: {
	onClick: () => void;
	onDelete: () => void;
	user: MinecraftCredentials;
	loggedIn?: boolean;
}) {
	return (
		<Button
			className={twMerge('w-full flex flex-row justify-between p-2 rounded-lg', !loggedIn && 'hover:bg-component-bg-hover active:bg-component-bg-pressed hover:text-fg-primary-hover')}
			color="ghost"
			onClick={onClick}
		>
			<div className="flex flex-1 flex-row justify-start gap-x-3">
				<div className="flex flex-1 flex-row justify-start gap-x-3">
					<AccountAvatar className="h-8 w-8 rounded-md" uuid={user.id} />
					<div className="flex flex-col items-center justify-center">
						<div className="flex flex-col items-start justify-between">
							<p className="h-[18px] font-semibold">{user.username}</p>
							{loggedIn && <p className="text-xs">Logged in</p>}
						</div>
					</div>
				</div>

				<div className="flex flex-row items-center">
					<ManageSkinButton profile={user} />
					<DeleteAccountButton onPress={onDelete} profile={user} />
				</div>
			</div>
		</Button>
	);
}
