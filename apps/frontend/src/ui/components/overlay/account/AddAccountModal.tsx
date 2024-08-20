import { type Context, Match, type ParentProps, type Resource, Switch, createContext, createSignal, useContext } from 'solid-js';
import { LinkExternal01Icon } from '@untitled-theme/icons-solid';
import type { MinecraftCredentials } from '@onelauncher/client/bindings';
import Modal, { type ModalProps, createModal } from '../Modal';
import { bridge } from '~imports';
import useCommand, { tryResult } from '~ui/hooks/useCommand';
import Button from '~ui/components/base/Button';

interface AccountControllerContextFunc {
	displayAddAccount: () => void;
	refetch: () => void;
	accounts: Resource<MinecraftCredentials[]>;
	defaultAccount: Resource<MinecraftCredentials | null>;
	setDefaultAccount: (uuid: string | null) => Promise<void>;
	removeAccount: (uuid: string, force?: boolean) => Promise<void>;
}

const AccountControllerContext = createContext<AccountControllerContextFunc>() as Context<AccountControllerContextFunc>;

export function AccountControllerProvider(props: ParentProps) {
	const [accounts, { refetch: refetchAccounts }] = useCommand(bridge.commands.getUsers);
	const [defaultAccount, { refetch: refetchDefaultAccount }] = useCommand(bridge.commands.getDefaultUser, true);

	const [deleteModalUuid, setDeleteModalUuid] = createSignal<string>();

	const addAccountModal = createModal(props => (
		<AddAccountModal {...props} refetch={refetch} />
	));

	const deleteAccountModal = createModal(props => (
		<Modal.Delete {...props} onDelete={() => _forceRemoveAccount(deleteModalUuid())} />
	));

	function refetch() {
		refetchAccounts();
		refetchDefaultAccount();
	}

	async function setDefaultAccount(uuid: string | null) {
		await tryResult(bridge.commands.setDefaultUser, uuid).then(refetch);
	}

	async function removeAccount(uuid: string, force?: boolean) {
		if (force !== true) {
			setDeleteModalUuid(uuid);
			deleteAccountModal.show();
		}
		else { await _forceRemoveAccount(uuid); }
	}

	async function _forceRemoveAccount(uuid: string | undefined) {
		if (uuid === undefined)
			return;

		await tryResult(bridge.commands.removeUser, uuid);
		refetch();
	}

	const func: AccountControllerContextFunc = {
		displayAddAccount: () => addAccountModal.show(),
		refetch,
		accounts,
		defaultAccount,
		setDefaultAccount,
		removeAccount,
	};

	return (
		<AccountControllerContext.Provider value={func}>
			{props.children}
		</AccountControllerContext.Provider>
	);
}

export function useAccountController() {
	return useContext(AccountControllerContext);
}

export default useAccountController;

enum ModalStage {
	Tasks,
	WaitingForCode,
	LoggingIn,
}

interface AddAccountModalProps extends ModalProps {
	refetch: () => any;
}

function AddAccountModal(p: AddAccountModalProps) {
	const [modalProps, props] = Modal.SplitProps(p);
	const [stage, setStage] = createSignal(ModalStage.Tasks);

	function start() {
		setStage(ModalStage.WaitingForCode);

		tryResult(bridge.commands.authLogin).finally(finish);
	}

	function finish() {
		modalProps.hide();
		props.refetch();
	}

	return (
		<Modal.Simple
			{...modalProps}
			title="Add Account"
			buttons={[
				<Button
					buttonStyle="secondary"
					children="Cancel"
					onClick={() => modalProps.hide()}
				/>,
				<Button
					buttonStyle="primary"
					children="Add"
					iconLeft={<LinkExternal01Icon />}
					onClick={start}
					disabled={stage() !== 0}
				/>,
			]}
		>
			<div class="max-w-120 flex flex-col gap-y-3 line-height-normal">
				<Switch>
					<Match when={stage() !== ModalStage.LoggingIn}>
						<p>
							Pressing the "Add" button will open your browser with a Microsoft login page.
							On this page, you login to your chosen Microsoft account and end up being asked whether you want to add the OneLauncher application.
						</p>
					</Match>

					<Match when={stage() === ModalStage.LoggingIn}>
						<p>Proceeding with Microsoft auth steps...</p>
					</Match>

				</Switch>
			</div>
		</Modal.Simple>
	);
};
