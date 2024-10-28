import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import { type Accessor, type Context, createContext, createEffect, createSignal, type JSX, type ParentProps, type Setter, splitProps, useContext } from 'solid-js';
import styles from './SelectList.module.scss';

interface SelectListContextHelpers {
	selected: Accessor<number[]>;
	setSelected: Setter<number[]>;
	select: (index: number) => void;
}
const SelectListContext = createContext<SelectListContextHelpers>() as Context<SelectListContextHelpers>;

function SelectListContextProvider(props: ParentProps & {
	onChanged?: ((index: number[]) => void) | undefined;
	multiple?: boolean | undefined;
	selected?: number[] | undefined;
}) {
	// eslint-disable-next-line solid/reactivity -- -
	const [selected, setSelected] = createSignal<number[]>(props.selected ?? []);

	createEffect(() => {
		props.onChanged?.(selected());
	});

	createEffect(() => {
		if (props.selected !== undefined)
			setSelected(props.selected);
	});

	const ctx = {
		selected,
		setSelected,
		select(index: number) {
			setSelected((prev) => {
				if (props.multiple) {
					if (prev.includes(index))
						return prev.filter(i => i !== index);

					return [...prev, index];
				}
				else {
					return [index];
				}
			});
		},
	};

	return (
		<SelectListContext.Provider value={ctx}>
			{props.children}
		</SelectListContext.Provider>
	);
}

function useSelectListContext() {
	return useContext(SelectListContext);
}

export type SelectListProps = Omit<JSX.HTMLAttributes<HTMLDivElement>, 'onChange'> & {
	onChange?: (index: number[]) => void;
	multiple?: boolean | undefined;
	selected?: number[] | undefined;
};

function SelectList(props: SelectListProps) {
	const [split, rest] = splitProps(props, ['class', 'onChange']);

	return (
		<SelectListContextProvider
			multiple={props.multiple}
			onChanged={props.onChange}
			selected={props.selected}
		>
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
	index: number;
};

SelectList.Row = (props: SelectListRowProps) => {
	const { selected, setSelected, select } = useSelectListContext();
	const [split, rest] = splitProps(props, ['index', 'class', 'onClick']);

	return (
		<div
			{...rest}
			class={`${styles.row} ${props.index !== undefined && selected().includes(props.index) ? styles.selected : ''} ${split.class || ''}`}
			onClick={(e) => {
				if (props.index !== undefined)
					if (e.shiftKey) {
						const indexes = selected();
						const last = indexes[indexes.length - 1]!;
						const start = last !== undefined ? Math.min(last, props.index) : props.index;
						const end = Math.max(last, props.index) || start;
						const newIndexes = Array.from({ length: end - start + 1 }, (_, i) => i + start);
						setSelected(prev => [
							...prev.filter(i => !newIndexes.includes(i)),
							...newIndexes,
						]);
					}
					else {
						select(split.index);
					}

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
