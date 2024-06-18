import { ChevronRightIcon, InfoCircleIcon } from '@untitled-theme/icons-solid';
import { For, Show, createSignal, mergeProps } from 'solid-js';
import Button from '~ui/components/base/Button';
import Tooltip from '~ui/components/base/Tooltip';
import PlayerHead from '~ui/components/game/PlayerHead';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Sidebar from '~ui/components/Sidebar';

const Accounts: {
	username: string;
	uuid: string;
}[] = [
	{
		username: 'Lynith',
		uuid: 'f247be7c-5b82-41c6-9148-793ded77e71f',
	},
	{
		username: 'Wyvest',
		uuid: 'a5331404-0e77-440e-8bef-24c071dac1ae',
	},
	{
		username: 'nextday',
		uuid: '000646f6-b779-419c-b5d6-2c5ba52419bf',
	},
	{
		username: 'xXfake_playerXx',
		uuid: '000646f6-b779-419c-b5d6-aaa',
	},
];

function SettingsAccounts() {
	const [current, setCurrent] = createSignal<string>();

	return (
		<Sidebar.Page>
			<h1>Accounts</h1>
			<ScrollableContainer>

				<For each={Accounts}>
					{account => (
						<AccountRow
							username={account.username}
							uuid={account.uuid}
							current={current() === account.uuid}
							onClick={setCurrent}
						/>
					)}
				</For>

			</ScrollableContainer>
		</Sidebar.Page>
	);
}

interface AccountRowProps {
	username: string;
	uuid: string;
	current?: boolean;
	onClick: (uuid: string) => any;
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
					buttonStyle="icon"
					children={<ChevronRightIcon />}
					onClick={(e) => {
						e.stopPropagation();
						// TODO: Display fullscreen profile page
					}}
				/>
			</div>
		</div>
	);
}

export default SettingsAccounts;
