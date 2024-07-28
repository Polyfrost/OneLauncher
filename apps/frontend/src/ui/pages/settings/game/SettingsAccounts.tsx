import { InfoCircleIcon, LinkExternal01Icon, Trash01Icon, UserPlus02Icon } from '@untitled-theme/icons-solid';
import { type Accessor, For, Match, type Setter, Show, Switch, createEffect, createSignal, mergeProps } from 'solid-js';
import { bridge } from '~imports';
import Button from '~ui/components/base/Button';
import Tooltip from '~ui/components/base/Tooltip';
import PlayerHead from '~ui/components/game/PlayerHead';
import Modal from '~ui/components/overlay/Modal';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Sidebar from '~ui/components/Sidebar';
import useCommand from '~ui/hooks/useCommand';

function SettingsAccounts() {
	const [current, setCurrent] = createSignal<string>();
	const [accounts, { refetch }] = useCommand(bridge.commands.getUsers);
	const [modalVisible, setModalVisible] = createSignal(false);

	createEffect(() => {
		const users = accounts();
		if (users === undefined || users.length < 1 || current() !== undefined)
			return;

		setCurrent(users[0]!.id);
	});

	function onDelete(uuid: string) {
		bridge.commands.removeUser(uuid)
			.finally(() => {
				refetch();
			});
	}

	function showModal() {
		setModalVisible(true);
	}

	return (
		<Sidebar.Page>
			<h1>Accounts</h1>

			<ScrollableContainer>
				<Switch>
					<Match when={accounts()?.length === 0}>
						<div class="flex flex-col h-full gap-y-4 max-h-64 justify-center items-center">
							<span class="text-lg font-bold uppercase text-fg-secondary">No accounts added.</span>
							<span class="text-xl font-bold">Add one with the Add Account button.</span>
						</div>
					</Match>
					<Match when={accounts()?.length !== 0}>
						<For each={accounts()}>
							{account => (
								<AccountRow
									username={account.username}
									uuid={account.id}
									current={current() === account.id}
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

			<AddAccountModal
				setVisible={setModalVisible}
				visible={modalVisible}
				refetch={refetch}
			/>

		</Sidebar.Page>
	);
}

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

		// tryResult(bridge.commands.beginMsa).then((res) => {
		// 	console.log(res);
		// });
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
