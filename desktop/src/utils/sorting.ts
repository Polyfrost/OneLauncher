import { type Accessor, type Setter, createSignal } from 'solid-js';

export type SortingFunction<T> = (a: T, b: T) => number;
export interface Sortables<T> { [key: string]: SortingFunction<T> }

type CreateSortable<T, S extends Sortables<T> = Sortables<T>> = [
	list: Accessor<T[]>,
	key: Accessor<keyof S>,
	setList: Setter<T[]>,
	setKey: Setter<keyof S>,
];

export function createSortable<T, S extends Sortables<T> = Sortables<T>>(initList: T[], initSortables: S): CreateSortable<T, S> {
	const sortables = initSortables;
	const [list, setList] = createSignal<T[]>(initList);
	const [key, setKey] = createSignal<keyof S>(Object.keys(sortables)[0]!);

	return [
		list,
		key,
		setList,
		setKey,
	];
}

export default createSortable;
