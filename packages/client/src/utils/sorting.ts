import { type Accessor, createSignal, type Setter } from 'solid-js';

export type SortingFunction<T> = (a: T, b: T) => number;
export type Sortables<T> = Record<string, SortingFunction<T>>;

interface CreateSortable<T, S extends Sortables<T> = Sortables<T>> {
	list: Accessor<T[]>;
	key: Accessor<keyof S>;
	setList: Setter<T[]>;
	setKey: (val: keyof S | number) => void;
	sortables: S;
}

export function createSortable<T, S extends Sortables<T> = Sortables<T>>(initList: T[], initSortables: S): CreateSortable<T, S> {
	const sortables = initSortables;
	const [list, setList] = createSignal<T[]>(initList);
	const [key, setKey] = createSignal<keyof S>(Object.keys(sortables)[0]!);

	return {
		list,
		key,
		setList,
		setKey: (val: keyof S | number) => {
			if (typeof val === 'string') {
				setKey(val);
			}
			else if (typeof val === 'number') {
				const key = Object.keys(sortables).at(val);
				if (key === undefined)
					throw new Error('Key cannot be undefined!');

				setKey(key);
			}
		},
		sortables,
	};
}

export default createSortable;
