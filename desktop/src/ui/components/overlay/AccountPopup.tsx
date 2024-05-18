import { LogOut01Icon, PlusIcon, Settings01Icon } from '@untitled-theme/icons-solid';
import Button from '../base/Button';
import PlayerHead from '../game/PlayerHead';
import useAccount from '../../hooks/useAccount';
import Popup from './Popup';

interface AccountComponentProps {
	username: string;
	uuid: string;
	loggedIn?: boolean;
}

function AccountComponent(props: AccountComponentProps) {
	return (
		<div class={`flex flex-row justify-between p-2 rounded-lg ${!props.loggedIn && 'hover:bg-gray-05 active:bg-gray-10 hover:text-fg-primary-hover'}`}>
			<div class="flex flex-row justify-start flex-1 gap-x-3">
				<PlayerHead class="w-8 h-8 rounded-md" uuid={props.uuid} />
				<div class="flex flex-col items-center justify-center">
					<div class="flex flex-col items-start justify-between">
						<p class="font-semibold h-[18px]">{props.username}</p>
						{props.loggedIn && <p class="text-xs">Logged in</p>}
					</div>
				</div>
			</div>
			{props.loggedIn && (
				<Button buttonStyle="icon">
					<LogOut01Icon class=" stroke-danger" />
				</Button>
			)}
		</div>
	);
}

function AccountPopup(props: Popup.PopupProps) {
	const loggedInAccount = useAccount();

	return (
		<Popup {...props}>
			<div class="bg-secondary rounded-xl border border-gray-10 w-72 p-2 shadow-md shadow-black/30">
				<div class="flex flex-col gap-y-2 text-fg-primary">
					<AccountComponent username={loggedInAccount.username} uuid={loggedInAccount.uuid} loggedIn />

					<div class="w-full h-px bg-gray-05 rounded-md" />

					<AccountComponent username="Caledonian" uuid="f247be7c5b8241c69148793ded77e71f" />
					<AccountComponent username="Caledonian" uuid="069a79f4-44e9-4726-a5be-fca90e38aaf5" />
					<AccountComponent username="Caledonian" uuid="f247be7c5b8241c69148793ded77e71f" />

					<div class="w-full h-px bg-gray-05 rounded-md" />

					<div class="flex flex-row justify-between">
						<div>
							<Button buttonStyle="ghost" iconLeft={<PlusIcon />}>Add Account</Button>
						</div>
						<div class="flex flex-row">
							<Button buttonStyle="icon" large>
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
