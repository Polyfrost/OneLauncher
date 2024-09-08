import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import { type Context, createContext, createEffect, createSignal, type JSX, type ParentProps, type Signal, splitProps, useContext } from 'solid-js';
import styles from './SelectList.module.scss';

type SelectListContextHelpers = Signal<number | undefined>;
const SelectListContext = createContext<SelectListContextHelpers>() as Context<SelectListContextHelpers>;

function SelectListContextProvider(props: ParentProps & {
	onChanged?: ((index: number | undefined) => any) | undefined;
}) {
	const [selected, setSelected] = createSignal<number | undefined>();

	createEffect(() => {
		props.onChanged?.(selected());
	});

	return (
		<SelectListContext.Provider value={[selected, setSelected]}>
			{props.children}
		</SelectListContext.Provider>
	);
}

function useSelectListContext() {
	return useContext(SelectListContext);
}

export type SelectListProps = Omit<JSX.HTMLAttributes<HTMLDivElement>, 'onChange'> & {
	onChange?: (index: number | undefined) => any;
};

function SelectList(props: SelectListProps) {
	const [split, rest] = splitProps(props, ['class', 'onChange']);
	return (
		<SelectListContextProvider onChanged={props.onChange}>
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
			class={`${styles.row} ${props.index !== undefined && selected() === props.index ? styles.selected : ''} ${split.class || ''}`}
			onClick={(e) => {
				if (props.index !== undefined)
					setSelected(props.index);

				if (typeof split.onClick === 'function')
					split.onClick(e);
			}}
			tabIndex={0}
		>
			{props.children}
		</div>
	);
};

export default SelectList;
