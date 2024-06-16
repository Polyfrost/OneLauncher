import { Index, type JSX, type ParentProps, children, createSignal, splitProps } from 'solid-js';
import { ChevronDownIcon } from '@untitled-theme/icons-solid';
import Popup from '../overlay/Popup';
import Button from './Button';
import styles from './Dropdown.module.scss';

export type DropdownProps = JSX.HTMLAttributes<HTMLDivElement>;

function Dropdown(props: DropdownProps) {
	const [split, rest] = splitProps(props, ['children', 'class']);
	const [visible, setVisible] = createSignal(false);
	const [selected, setSelected] = createSignal<number>(0);

	let ref!: HTMLDivElement;

	const items = () => children(() => split.children).toArray();

	function select(index: number) {
		setSelected(index);
		setVisible(false);
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
					class="flex-1 h-full mt-px overflow-hidden"
				>
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
