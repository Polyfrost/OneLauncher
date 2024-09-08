import { ChevronDownIcon, ChevronUpIcon } from '@untitled-theme/icons-solid';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import { type Accessor, children, createEffect, createSignal, Index, type JSX, Match, type ParentProps, type ResolvedJSXElement, type Setter, Show, splitProps, Switch } from 'solid-js';
import Popup from '../overlay/Popup';
import Button from './Button';
import styles from './Dropdown.module.scss';

export type DropdownProps = Omit<JSX.HTMLAttributes<HTMLDivElement>, 'onChange'> & {
	onChange?: (selected: number) => any;
	text?: string;
	selected?: Accessor<number>;
	dropdownClass?: string;
	listToolRow?: () => JSX.Element;
	component?: (
		props: {
			visible: Accessor<boolean>;
			setVisible: Setter<boolean>;
			children: () => ResolvedJSXElement[];
		}
	) => JSX.Element;
};

function Dropdown(props: DropdownProps) {
	const [split, rest] = splitProps(props, ['disabled', 'children', 'class', 'text', 'onChange', 'selected', 'component', 'dropdownClass', 'listToolRow']);
	const [visible, setVisible] = createSignal(false);
	const [selected, setSelected] = createSignal(0);

	createEffect(() => {
		if (split.selected)
			setSelected(split.selected());
	});

	let ref!: HTMLDivElement;

	const items = () => children(() => split.children).toArray();

	function select(index: number) {
		setSelected(index);
		setVisible(false);

		if (split.onChange)
			split.onChange(index);
	}

	const Component = () => (
		split.component
			? split.component({ visible, setVisible, children: items })
			: (
					<Button
						buttonStyle="secondary"
						class="h-full w-full"
						iconRight={<ChevronDownIcon />}
						onClick={() => setVisible(true)}
					>
						<div
							class="h-full flex flex-1 flex-row items-center gap-1 overflow-hidden text-nowrap"
						>
							<span>{split.text}</span>
							{items()[selected()]}
						</div>
					</Button>
				)
	);

	return (
		<div class={`${styles.dropdown} ${split.class || ''}`} data-disabled={split.disabled || false} ref={ref} {...rest}>
			<Component />

			<Popup
				mount={ref}
				ref={(ref) => {
					ref.classList.add('mt-1', 'w-full');
					if (split.dropdownClass)
						ref.classList.add(...split.dropdownClass.split(' '));
				}}
				setVisible={setVisible}
				visible={visible}
			>
				<div class={`${styles.dropdown_elements_container} ${visible() ? styles.visible : ''}`}>
					<OverlayScrollbarsComponent class={`${styles.list} ${styles.dropdown_element}`}>
						{/* TODO(perf): Optimise */}
						<Index each={items()}>
							{(item, index) => (
								<div onClick={() => select(index)}>
									<div class="flex flex-row items-center justify-between gap-2 rounded-lg p-2 hover:bg-gray-05">
										{item()}
										<div class={styles.selected! + (selected() === index ? ` ${styles.visible}` : '')} />
									</div>
								</div>
							)}
						</Index>
					</OverlayScrollbarsComponent>
					<Show when={split.listToolRow !== undefined}>
						<div class={styles.dropdown_element}>
							{split.listToolRow!()}
						</div>
					</Show>
				</div>
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
			component={props => (
				<Button
					buttonStyle="iconSecondary"
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
					onClick={() => props.setVisible(true)}
				/>
			)}
			dropdownClass="w-max!"
			{...rest}
		/>
	);
};

export default Dropdown;
