import type { MinecraftCredentials } from '@/bindings.gen';
import { bindings } from '@/main';
import { useCommand, useCommandMut } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { Link } from '@tanstack/react-router';
import { PlusIcon, Settings01Icon } from '@untitled-theme/icons-react';
import { DialogTrigger } from 'react-aria-components';
import { twMerge } from 'tailwind-merge';
import { AccountAvatar } from '../AccountAvatar';
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

	return (
		<Popup placement="top left">

			<div className="min-w-3xs">
				{defaultUser.data && (
					<div>
						<AccountEntry
							loggedIn
							onClick={() => { }}
							user={defaultUser.data}
						/>
					</div>
				)}

				{users.data?.filter(user => user.id !== defaultUser.data?.id).map(user => (
					<div key={user.id}>
						<AccountEntry
							onClick={() => setDefaultUser(user)}
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
	user,
	loggedIn = false,
}: {
	onClick: () => void;
	user: MinecraftCredentials;
	loggedIn?: boolean;
}) {
	return (
		<div
			className={twMerge('flex flex-row justify-between p-2 rounded-lg', !loggedIn && 'hover:bg-component-bg-hover active:bg-component-bg-pressed hover:text-fg-primary-hover')}
			onClick={onClick}
		>
			<div className="flex flex-1 flex-row justify-start gap-x-3">
				<AccountAvatar className="h-8 w-8 rounded-md" uuid={user.id} />
				<div className="flex flex-col items-center justify-center">
					<div className="flex flex-col items-start justify-between">
						<p className="h-[18px] font-semibold">{user.username}</p>
						{loggedIn && <p className="text-xs">Logged in</p>}
					</div>
				</div>
			</div>
		</div>
	);
}
