import { For, Show } from 'solid-js';
import { PlusIcon, Settings01Icon } from '@untitled-theme/icons-solid';
import { useNavigate } from '@solidjs/router';
import Button from '../../base/Button';
import PlayerHead from '../../game/PlayerHead';
import Popup from '../Popup';
import useAccountController from './AddAccountModal';
import type { MinecraftCredentials } from '~bindings';

interface AccountComponentProps {
	account: MinecraftCredentials | null | undefined;
	loggedIn?: boolean;
}

function AccountComponent(props: AccountComponentProps) {
	const controller = useAccountController();

	function login() {
		if (props.loggedIn === true)
			return;

		if (props.account)
			controller.setDefaultAccount(props.account.id);
	}

	return (
		<Show when={props.account}>
			<div
				onClick={login}
				class={`flex flex-row justify-between p-2 rounded-lg ${props.loggedIn !== true && 'hover:bg-gray-05 active:bg-gray-10 hover:text-fg-primary-hover'}`}
			>
				<div class="flex flex-row justify-start flex-1 gap-x-3">
					<PlayerHead class="w-8 h-8 rounded-md" uuid={props.account!.id} />
					<div class="flex flex-col items-center justify-center">
						<div class="flex flex-col items-start justify-between">
							<p class="font-semibold h-[18px]">{props.account!.username}</p>
							{props.loggedIn && <p class="text-xs">Logged in</p>}
						</div>
					</div>
				</div>

				{/* <Show when={props.loggedIn}>
					<Button
						buttonStyle="icon"
						onClick={() => controller.setDefaultAccount(null)}
						children={<LogOut01Icon class=" stroke-danger" />}
					/>
				</Show> */}
			</div>
		</Show>
	);
}

function AccountPopup(props: Popup.PopupProps) {
	const controller = useAccountController();

	const filteredAccounts = () => controller.accounts()?.filter(account => account.id !== controller.defaultAccount()?.id) ?? [];

	const navigate = useNavigate();

	return (
		<Popup {...props}>
			<div class="bg-secondary rounded-xl border border-gray-10 w-72 p-2 shadow-md shadow-black/30">
				<div class="flex flex-col gap-y-2 text-fg-primary">
					<Show when={controller.defaultAccount() !== null || controller.defaultAccount() !== undefined}>
						<AccountComponent
							account={controller.defaultAccount()}
							loggedIn
						/>
					</Show>

					<Show when={filteredAccounts().length !== 0}>
						<For each={filteredAccounts()}>
							{account => (
								<AccountComponent account={account} />
							)}
						</For>
					</Show>
					<div class="w-full h-px bg-gray-05 rounded-md" />

					<div class="flex flex-row justify-between">
						<div>
							<Button
								buttonStyle="ghost"
								iconLeft={<PlusIcon />}
								onClick={() => {
									props.setVisible(false);
									controller.displayAddAccount();
								}}
							>
								Add Account
							</Button>
						</div>
						<div class="flex flex-row">
							<Button
								buttonStyle="icon"
								large
								onClick={() => {
									props.setVisible(false);
									navigate('/settings/accounts');
								}}
							>
								<Settings01Icon class="stroke-fg-primary" />
							</Button>
						</div>
					</div>

				</div>
			</div>
		</Popup>
	);
}

export default AccountPopup;
