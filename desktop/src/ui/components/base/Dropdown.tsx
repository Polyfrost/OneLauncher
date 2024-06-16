import { Index, type JSX, type ParentProps, children, createSignal } from 'solid-js';
import { ChevronDownIcon } from '@untitled-theme/icons-solid';
import Popup from '../overlay/Popup';
import Button from './Button';
import styles from './Dropdown.module.scss';

export type DropdownProps = ParentProps;

function Dropdown(props: DropdownProps) {
	const [visible, setVisible] = createSignal(false);
	const [selected, setSelected] = createSignal<number>(0);
	let ref!: HTMLDivElement;

	const items = () => children(() => props.children).toArray();

	function select(index: number) {
		setSelected(index);
		setVisible(false);
	}

	return (
		<div ref={ref} class={styles.dropdown}>
			<Button
				class="h-full"
				buttonStyle="secondary"
				onClick={() => setVisible(true)}
				iconRight={<ChevronDownIcon />}
			>
				<div
					class="w-full h-full overflow-hidden mt-px"
				>
					<div
						class="flex flex-col justify-start relative gap-4"
						style={{
							transform: `translateY(-${(selected() * 16) * 2}px)`,
						}}
					>
						{items()}
					</div>
				</div>
			</Button>

			<Popup
				mount={ref}
				visible={visible}
				setVisible={setVisible}
				ref={ref => ref.classList.add('mt-1', 'left-0', 'right-0')}
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

interface DropdownRowProps {
	iconLeft?: JSX.Element;
};

Dropdown.Row = function (props: ParentProps) {
	return (
		<div class={styles.row}>
			{props.children}
		</div>
	);
};

export default Dropdown;
