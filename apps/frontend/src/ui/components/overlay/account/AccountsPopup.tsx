import { For, Show } from 'solid-js';
import { PlusIcon, Settings01Icon } from '@untitled-theme/icons-solid';
import { useNavigate } from '@solidjs/router';
import Button from '../../base/Button';
import PlayerHead from '../../game/PlayerHead';
import Popup, { type PopupProps } from '../Popup';
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
				<div class="flex flex-1 flex-row justify-start gap-x-3">
					<PlayerHead class="h-8 w-8 rounded-md" uuid={props.account!.id} />
					<div class="flex flex-col items-center justify-center">
						<div class="flex flex-col items-start justify-between">
							<p class="h-[18px] font-semibold">{props.account!.username}</p>
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

function AccountPopup(props: PopupProps) {
	const controller = useAccountController();

	const filteredAccounts = () => controller.accounts()?.filter(account => account.id !== controller.defaultAccount()?.id) ?? [];

	const navigate = useNavigate();

	return (
		<Popup {...props}>
			<div class="w-72 border border-gray-10 rounded-xl bg-page-elevated p-2 shadow-black/30 shadow-md">
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
					<div class="h-px w-full rounded-md bg-gray-05" />

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
