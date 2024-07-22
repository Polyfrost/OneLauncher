import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import { type Context, type JSX, type ParentProps, type Signal, createContext, createSignal, splitProps, useContext } from 'solid-js';
import styles from './SelectList.module.scss';

type SelectListContextHelpers = Signal<number | undefined>;
const SelectListContext = createContext<SelectListContextHelpers>() as Context<SelectListContextHelpers>;

function SelectListContextProvider(props: ParentProps) {
	const [selected, setSelected] = createSignal<number>();

	return (
		<SelectListContext.Provider value={[selected, setSelected]}>
			{props.children}
		</SelectListContext.Provider>
	);
}

function useSelectListContext() {
	return useContext(SelectListContext);
}

export type SelectListProps = JSX.HTMLAttributes<HTMLDivElement>;

function SelectList(props: SelectListProps) {
	const [split, rest] = splitProps(props, ['class']);
	return (
		<SelectListContextProvider>
			<div {...rest} class={`${styles.select_list} ${split.class || ''}`}>
				<OverlayScrollbarsComponent class="max-h-full">
					<div>
						{props.children}
					</div>
				</OverlayScrollbarsComponent>
			</div>
		</SelectListContextProvider>
	);
}

export type SelectListRowProps = JSX.HTMLAttributes<HTMLDivElement> & {
	index?: number;
};

SelectList.Row = (props: SelectListRowProps) => {
	const [selected, setSelected] = useSelectListContext();
	const [split, rest] = splitProps(props, ['index', 'class', 'onClick']);

	return (
		<div
			{...rest}
			tabIndex={0}
			onClick={(e) => {
				if (props.index !== undefined)
					setSelected(props.index);

				if (split.onClick)
				// @ts-expect-error -- la
					split.onClick(e);
			}}
			class={`${styles.row} ${props.index !== undefined && selected() === props.index ? styles.selected : ''} ${split.class || ''}`}
		>
			{props.children}
		</div>
	);
};

export default SelectList;
