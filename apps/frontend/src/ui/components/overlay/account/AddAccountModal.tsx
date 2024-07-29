import { type Accessor, type Context, Match, type ParentProps, type Resource, type Setter, Switch, createContext, createSignal, useContext } from 'solid-js';
import { LinkExternal01Icon } from '@untitled-theme/icons-solid';
import Modal from '../Modal';
import type { MinecraftCredentials } from '~bindings';
import { bridge } from '~imports';
import useCommand, { tryResult } from '~ui/hooks/useCommand';
import Button from '~ui/components/base/Button';

interface AccountControllerContextFunc {
	displayAddAccount: () => void;
	refetch: () => void;
	accounts: Resource<MinecraftCredentials[]>;
	defaultAccount: Resource<MinecraftCredentials | null>;
	setDefaultAccount: (uuid: string) => Promise<void>;
	removeAccount: (uuid: string, force?: boolean) => Promise<void>;
}

const AccountControllerContext = createContext<AccountControllerContextFunc>() as Context<AccountControllerContextFunc>;

export function AccountControllerProvider(props: ParentProps) {
	const [accounts, { refetch: refetchAccounts }] = useCommand(bridge.commands.getUsers);
	const [defaultAccount, { refetch: refetchDefaultAccount }] = useCommand(bridge.commands.getDefaultUser);
	const [visible, setVisible] = createSignal(false);

	const [deleteModalUuid, setDeleteModalUuid] = createSignal<string>();
	const deleteModalVisible = () => deleteModalUuid() !== undefined;

	function refetch() {
		refetchAccounts();
		refetchDefaultAccount();
	}

	async function setDefaultAccount(uuid: string) {
		await tryResult(bridge.commands.setDefaultUser, uuid);
	}

	async function removeAccount(uuid: string, force?: boolean) {
		if (force !== true)
			setDeleteModalUuid(uuid);
		else
			await _forceRemoveAccount(uuid);
	}

	async function _forceRemoveAccount(uuid: string | undefined) {
		if (uuid === undefined)
			return;

		await tryResult(bridge.commands.removeUser, uuid);
		refetch();
	}

	const func: AccountControllerContextFunc = {
		displayAddAccount: () => setVisible(true),
		refetch,
		accounts,
		defaultAccount,
		setDefaultAccount,
		removeAccount,
	};

	return (
		<AccountControllerContext.Provider value={func}>
			{props.children}
			<AddAccountModal visible={visible} setVisible={setVisible} refetch={refetch} />

			<Modal.Delete
				setVisible={value => setDeleteModalUuid(value ? deleteModalUuid() : undefined)}
				visible={deleteModalVisible}
				onDelete={() => _forceRemoveAccount(deleteModalUuid())}
			/>
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

interface AddAccountModalProps {
	visible: Accessor<boolean>;
	setVisible: Setter<boolean>;
	refetch: () => any;
}

function AddAccountModal(props: AddAccountModalProps) {
	const [stage, setStage] = createSignal(ModalStage.Tasks);

	function start() {
		setStage(ModalStage.WaitingForCode);

		tryResult(bridge.commands.authLogin).finally(finish);
	}

	function finish() {
		props.setVisible(false);
		props.refetch();
	}

	return (
		<Modal.Simple
			title="Add Account"
			visible={props.visible}
			setVisible={props.setVisible}
			buttons={[
				<Button
					buttonStyle="secondary"
					children="Cancel"
					onClick={() => props.setVisible(false)}
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
			<div class="flex flex-col gap-y-3 max-w-120 line-height-normal">
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
}
