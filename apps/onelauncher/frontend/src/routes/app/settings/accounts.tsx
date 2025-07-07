import PlayerHead from '@/components/content/PlayerHead';
import ScrollableContainer from '@/components/ScrollableContainer';
import { bindings } from '@/main';
import { useCommand } from '@onelauncher/common';
import { Button, Show, Tooltip } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { InfoCircleIcon, Trash01Icon, UserPlus02Icon } from '@untitled-theme/icons-react';
import { useState } from 'react';
import Sidebar from './route';

export const Route = createFileRoute('/app/settings/accounts')({
	component: RouteComponent,
});

interface Account {
	id: string;
	username: string;
}

function RouteComponent() {
	const { data: usersData, isLoading: usersLoading } = useCommand('getUsers', bindings.core.getUsers);
	const { data: defaultUserData, isLoading: defaultUserLoading } = useCommand('getDefaultUser', () => bindings.core.getDefaultUser(false));

	const addAccount = useCommand('openMsaLogin', bindings.core.openMsaLogin, {
		enabled: false,
		subscribed: false,
	});

	if (usersLoading || defaultUserLoading)
		return (
			<Sidebar.Page>
				<h1>Accounts</h1>
				<p>Loading accounts...</p>
			</Sidebar.Page>
		);

	return (
		<Sidebar.Page>
			<h1>Accounts</h1>
			<ScrollableContainer>
				<div className="h-full space-y-2">
					<Show fallback={<p>No accounts found.</p>} when={usersData && usersData.length > 0}>
						{usersData?.map(account => (
							<AccountRow
								account={account}
								isCurrent={account.id === defaultUserData?.id}
								key={account.id}
							/>
						))}
					</Show>
				</div>
			</ScrollableContainer>

			<div className="mt-auto pt-2 flex flex-row items-end justify-end">
				<Button color="primary" onClick={() => addAccount.refetch()}>
					<UserPlus02Icon />
					{' '}
					Add Account
				</Button>
			</div>
		</Sidebar.Page>
	);
}

interface AccountRowProps {
	account: Account;
	isCurrent?: boolean;
};

function AccountRow({ account, isCurrent }: AccountRowProps) {
	const [errored, setErrored] = useState(false);

	const setDefaultUserCommand = useCommand(
		'setDefaultUser',
		() => bindings.core.setDefaultUser(account.id),
		{
			enabled: false,
			subscribed: false,
		},
	);

	const removeUserCommand = useCommand(
		'removeUser',
		() => bindings.core.removeUser(account.id),
		{
			enabled: false,
			subscribed: false,
		},
	);

	const handleSetDefault = () => {
		setDefaultUserCommand.refetch();
	};

	const handleDelete = (e: React.MouseEvent) => {
		e.stopPropagation();
		removeUserCommand.refetch();
	};

	return (
		<div
			className={`flex flex-row bg-component-bg hover:bg-component-bg-hover active:bg-component-bg-pressed rounded-xl gap-3.5 p-4 items-center box-border border ${isCurrent ? 'border-brand' : 'border-transparent'} cursor-pointer`} // Added cursor-pointer
			onClick={handleSetDefault}
			onKeyDown={(e) => {
				if (e.key === 'Enter' || e.key === ' ')
					handleSetDefault();
			}}
			role="button"
			tabIndex={0}
		>
			<div className="h-12 w-12 flex items-center justify-center flex-shrink-0">
				<PlayerHead
					className="h-12 w-12 rounded-md"
					onError={() => setErrored(true)}
					uuid={account.id}
				/>
			</div>

			<div className={`flex flex-col gap-1 flex-1 min-w-0 ${errored ? 'text-danger' : ''}`}>
				<div className="flex flex-row items-center gap-1">
					<h3 className="text-lg font-medium truncate">{account.username}</h3>
					<Show when={errored}>
						<Tooltip
							text="Could not fetch this account's game profile"
						>
							<InfoCircleIcon className="h-4 w-4 flex-shrink-0" />
						</Tooltip>
					</Show>
				</div>
				<p className="text-xs text-fg-secondary truncate">{account.id}</p>
			</div>

			<div className="ml-auto flex-shrink-0">
				<Button
					aria-label={`Remove account ${account.username}`}
					color="danger"
					onClick={handleDelete}
					size="icon"
				>
					<Trash01Icon />
				</Button>
			</div>
		</div>
	);
}
