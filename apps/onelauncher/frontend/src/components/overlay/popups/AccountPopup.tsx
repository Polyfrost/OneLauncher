import type { MinecraftCredentials } from '@/bindings.gen';
import PlayerHead from '@/components/content/PlayerHead';
import { bindings } from '@/main';
import { useCommand } from '@onelauncher/common';
import { Button, ContextMenu } from '@onelauncher/common/components';
import { Link } from '@tanstack/react-router';
import { PlusIcon, Settings01Icon } from '@untitled-theme/icons-react';
import { useRef, useState } from 'react';
import { twMerge } from 'tailwind-merge';

function AccountPopup() {
	const users = useCommand('getUsers', bindings.core.getUsers);
	const defaultUser = useCommand('getDefaultUser', () => bindings.core.getDefaultUser(false));

	const addAccount = useCommand('openMsaLogin', bindings.core.openMsaLogin, {
		enabled: false,
		subscribed: false,
	});

	const setDefaultUser = (user: MinecraftCredentials) => {
		bindings.core.setDefaultUser(user.id);
	};

	return (
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
					<Button color="ghost" onClick={() => addAccount.refetch()}>
						<PlusIcon />
						Add Account
					</Button>
				</div>
				<div className="flex flex-row">
					<Link to="/app/settings/accounts">
						<Button color="ghost" size="iconLarge">
							<Settings01Icon className="stroke-fg-primary" />
						</Button>
					</Link>
				</div>
			</div>
		</div>
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
	const ref = useRef<HTMLDivElement>(null);
	const [isOpen, setOpen] = useState(false);

	const removeUserCommand = useCommand(
		'removeUser',
		() => bindings.core.removeUser(user.id),
		{
			enabled: false,
			subscribed: false,
		},
	);

	return (
		<div
			className={twMerge('flex flex-row justify-between p-2 rounded-lg', !loggedIn && 'hover:bg-component-bg-hover active:bg-component-bg-pressed hover:text-fg-primary-hover')}
			onClick={onClick}
			ref={ref}
		>
			<div className="flex flex-1 flex-row justify-start gap-x-3">
				<PlayerHead className="h-8 w-8 rounded-md" uuid={user.id} />
				<div className="flex flex-col items-center justify-center">
					<div className="flex flex-col items-start justify-between">
						<p className="h-[18px] font-semibold">{user.username}</p>
						{loggedIn && <p className="text-xs">Logged in</p>}
					</div>
				</div>
			</div>

			<ContextMenu
				isOpen={isOpen}
				setOpen={setOpen}
				triggerRef={ref}
			>
				<ContextMenu.Item className="text-red-500 rounded-sm px-3 py-1 hover:bg-component-bg-hover" onAction={removeUserCommand.refetch}>
					Delete
				</ContextMenu.Item>
			</ContextMenu>
		</div>
	);
}
