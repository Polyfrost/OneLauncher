import { Index, type JSX, type ParentProps, children, createSignal, splitProps } from 'solid-js';
import { ChevronDownIcon } from '@untitled-theme/icons-solid';
import Popup from '../overlay/Popup';
import Button from './Button';
import styles from './Dropdown.module.scss';

export type DropdownProps = Omit<JSX.HTMLAttributes<HTMLDivElement>, 'onChange'> & {
	onChange?: (selected: number) => any;
	text?: string;
	selected?: number;
};

function Dropdown(props: DropdownProps) {
	const [split, rest] = splitProps(props, ['children', 'class', 'text', 'onChange', 'selected']);
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

	return (
		<div ref={ref} class={`${styles.dropdown} ${split.class}`} {...rest}>
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

			<Popup
				mount={ref}
				visible={visible}
				setVisible={setVisible}
				ref={ref => ref.classList.add('mt-1', 'w-full')}
			>
				<div class="bg-secondary rounded-lg border border-gray-05 p-1 shadow-md shadow-black/30">
					<div class="flex flex-col gap-2">
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
					</div>
				</div>
			</Popup>
		</div>
	);
}

Dropdown.Row = function (props: ParentProps) {
	return (
		<div class={styles.row}>
			{props.children}
		</div>
	);
};

export default Dropdown;
