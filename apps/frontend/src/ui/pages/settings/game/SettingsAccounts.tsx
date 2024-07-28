import { InfoCircleIcon, Trash01Icon, UserPlus02Icon } from '@untitled-theme/icons-solid';
import { For, Match, Show, Switch, createSignal, mergeProps } from 'solid-js';
import Button from '~ui/components/base/Button';
import Tooltip from '~ui/components/base/Tooltip';
import PlayerHead from '~ui/components/game/PlayerHead';
import useAccountController from '~ui/components/overlay/account/AddAccountModal';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Sidebar from '~ui/components/Sidebar';

function SettingsAccounts() {
	const controller = useAccountController();

	function setCurrent(uuid: string) {
		controller.setDefaultAccount(uuid);
	}

	function onDelete(uuid: string) {
		controller.removeAccount(uuid);
	}

	function showModal() {
		controller.displayAddAccount();
	}

	return (
		<Sidebar.Page>
			<h1>Accounts</h1>

			<ScrollableContainer>
				<Switch>
					<Match when={controller.accounts()?.length === 0}>
						<div class="flex flex-col h-full gap-y-4 max-h-64 justify-center items-center">
							<span class="text-lg font-bold uppercase text-fg-secondary">No accounts added.</span>
							<span class="text-xl font-bold">Add one with the Add Account button.</span>
						</div>
					</Match>
					<Match when={controller.accounts()?.length !== 0}>
						<For each={controller.accounts()}>
							{account => (
								<AccountRow
									username={account.username}
									uuid={account.id}
									current={controller.defaultAccount()?.id === account.id}
									onClick={setCurrent}
									onDelete={onDelete}
								/>
							)}
						</For>
					</Match>
				</Switch>
			</ScrollableContainer>

			<div class="flex flex-row justify-end items-end mt-2">
				<Button
					buttonStyle="primary"
					iconLeft={<UserPlus02Icon />}
					children="Add Account"
					onClick={showModal}
				/>
			</div>
		</Sidebar.Page>
	);
}

interface AccountRowProps {
	username: string;
	uuid: string;
	current?: boolean;
	onClick: (uuid: string) => any;
	onDelete: (uuid: string) => any;
};

function AccountRow(props: AccountRowProps) {
	const defaultProps = mergeProps({ current: false }, props);
	const [errored, setErrored] = createSignal(false);

	return (
		<div
			onClick={() => props.onClick(props.uuid)}
			class={`flex flex-row bg-component-bg hover:bg-component-bg-hover active:bg-component-bg-pressed rounded-xl gap-3.5 p-4 items-center box-border border ${defaultProps.current ? 'border-brand' : 'border-transparent'}`}
		>
			<div class="flex justify-center items-center h-12 w-12">
				<PlayerHead
					class="w-12 h-12 rounded-md"
					uuid={props.uuid}
					onError={() => setErrored(true)}
				/>
			</div>

			<div class={`flex flex-col gap-2 flex-1 ${errored() ? 'text-danger' : ''}`}>
				<div class="flex flex-row items-center gap-1">
					<h3 class="text-xl">{props.username}</h3>
					<Show when={errored()}>
						<Tooltip
							text="Could not fetch this account's game profile"
						>
							<InfoCircleIcon class="w-4 h-4" />
						</Tooltip>
					</Show>
				</div>
				<p class="text-wrap text-sm text-fg-secondary">{props.uuid}</p>
			</div>

			<div class="">
				<Button
					buttonStyle="iconDanger"
					children={<Trash01Icon />}
					onClick={(e) => {
						e.stopPropagation();
						props.onDelete(props.uuid);
					}}
				/>
			</div>
		</div>
	);
}

export default SettingsAccounts;
