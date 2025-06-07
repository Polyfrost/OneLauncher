import type { MinecraftCredentials } from '@/bindings.gen';
import PlayerHead from '@/components/content/PlayerHead';
import { bindings } from '@/main';
import { useCommand } from '@onelauncher/common';
import { Button, Menu } from '@onelauncher/common/components';
import { PlusIcon, Settings01Icon } from '@untitled-theme/icons-react';
import { Separator } from 'react-aria-components';
import { twMerge } from 'tailwind-merge';

function AccountPopup() {
	const users = useCommand('getUsers', bindings.core.getUsers);
	const defaultUser = useCommand('getDefaultUser', () => bindings.core.getDefaultUser(false));

	const setDefaultUser = (user: MinecraftCredentials) => {
		bindings.core.setDefaultUser(user.id);
	};

	return (
		<Menu className="min-w-3xs">
			{defaultUser.data && (
				<Menu.Item>
					<AccountEntry
						loggedIn
						onClick={() => {}}
						user={defaultUser.data}
					/>
				</Menu.Item>
			)}

			{users.data?.filter(user => user.id === defaultUser.data?.id).map(user => (
				<Menu.Item key={user.id}>
					<AccountEntry
						onClick={() => setDefaultUser(user)}
						user={user}
					/>
				</Menu.Item>
			))}

			{(users.data?.length ?? 0) > 0 && (
				<Separator />
			)}

			<Menu.Item className="flex flex-row justify-between">
				<div>
					<Button color="ghost">
						<PlusIcon />
						Add Account
					</Button>
				</div>
				<div className="flex flex-row">
					<Button color="ghost" size="iconLarge">
						<Settings01Icon className="stroke-fg-primary" />
					</Button>
				</div>
			</Menu.Item>
		</Menu>
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
				<PlayerHead className="h-8 w-8 rounded-md" uuid={user.id} />
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
