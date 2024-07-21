import { type Accessor, Index, type JSX, Match, type ParentProps, type ResolvedJSXElement, type Setter, Switch, children, createSignal, splitProps } from 'solid-js';
import { ChevronDownIcon, ChevronUpIcon } from '@untitled-theme/icons-solid';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import Popup from '../overlay/Popup';
import Button from './Button';
import styles from './Dropdown.module.scss';

export type DropdownProps = Omit<JSX.HTMLAttributes<HTMLDivElement>, 'onChange'> & {
	onChange?: (selected: number) => any;
	text?: string;
	selected?: number;
	dropdownClass?: string;
	component?: (
		props: {
			visible: Accessor<boolean>;
			setVisible: Setter<boolean>;
			children: () => ResolvedJSXElement[];
		}
	) => JSX.Element;
};

function Dropdown(props: DropdownProps) {
	const [split, rest] = splitProps(props, ['children', 'class', 'text', 'onChange', 'selected', 'component', 'dropdownClass']);
	const [visible, setVisible] = createSignal(false);

	// eslint-disable-next-line solid/reactivity -- todo
	const [selected, setSelected] = createSignal<number>(split.selected || 0);

	let ref!: HTMLDivElement;

	const items = () => children(() => split.children).toArray();

	function select(index: number) {
		setSelected(index);
		setVisible(false);

		if (split.onChange)
			split.onChange(selected());
	}

	const Component = () => (
		split.component
			? split.component({ visible, setVisible, children: items })
			: (
					<Button
						class="h-full w-full"
						buttonStyle="secondary"
						onClick={() => setVisible(true)}
						iconRight={<ChevronDownIcon />}
					>
						<div
							class="flex-1 flex flex-row items-center text-nowrap gap-1 h-full overflow-hidden"
						>
							<span>{split.text}</span>
							{items()[selected()]}
						</div>
					</Button>
				)
	);

	return (
		<div ref={ref} class={`${styles.dropdown} ${split.class || ''}`} {...rest}>
			<Component />

			<Popup
				mount={ref}
				visible={visible}
				setVisible={setVisible}
				ref={(ref) => {
					ref.classList.add('mt-1', 'w-full');
					if (split.dropdownClass)
						ref.classList.add(...split.dropdownClass.split(' '));
				}}
			>
				<OverlayScrollbarsComponent class={`${styles.list} ${visible() ? styles.visible : ''}`}>
					<Index each={items()}>
						{(item, index) => (
							<div onClick={() => select(index)}>
								<div class="hover:bg-gray-05 p-2 rounded-lg flex flex-row gap-2 justify-between items-center">
									{item()}
									<div class={styles.selected! + (selected() === index ? ` ${styles.visible}` : '')} />
								</div>
							</div>
						)}
					</Index>
				</OverlayScrollbarsComponent>
			</Popup>
		</div>
	);
}

Dropdown.Row = function (props: ParentProps) {
	return (
	// TODO(a11y): tabIndex
		<div class={styles.row}>
			{props.children}
		</div>
	);
};

type MinimalDropdownProps = Omit<DropdownProps, 'component' | 'icon'> & {
	icon?: JSX.Element;
};

Dropdown.Minimal = function (props: MinimalDropdownProps) {
	const [split, rest] = splitProps(props, ['icon']);
	return (
		<Dropdown
			class="w-auto!"
			dropdownClass="w-max!"
			component={props => (
				<Button
					onClick={() => props.setVisible(true)}
					children={(
						<Switch>
							<Match when={split.icon}>
								{split.icon}
							</Match>
							<Match when={props.visible() === true}>
								<ChevronUpIcon />
							</Match>
							<Match when={props.visible() !== true}>
								<ChevronDownIcon />
							</Match>
						</Switch>
					)}
					buttonStyle="iconSecondary"
				/>
			)}
			{...rest}
		/>
	);
};

export default Dropdown;
