import type { DragEventHandler, Id } from '@thisbeyond/solid-dnd';
import { DragDropProvider, DragDropSensors, DragOverlay, SortableProvider, closestCenter, createSortable, useDragDropContext } from '@thisbeyond/solid-dnd';
import type { JSX } from 'solid-js';
import { For, createSignal } from 'solid-js';

declare module 'solid-js' {
	// eslint-disable-next-line ts/no-namespace
	namespace JSX {
		interface Directives {
			sortable: any;
		}
	}
}

interface ItemProps {
	item: boolean | Id | Node | JSX.ArrayElement | null | undefined;
};

function Item(props: ItemProps) {
	const id = () => props.item as Id;
	// eslint-disable-next-line solid/reactivity
	const sortable = createSortable(id());

	// @ts-expect-error - `state` is not defined in the type definition
	const [state] = useDragDropContext();

	return (
		<div
			use:sortable
			class="sortable"
			classList={{
				'opacity-25': sortable.isActiveDraggable,
				'transition-transform': !!state.active.draggable,
			}}
		>
			{props.item}
		</div>
	);
}

function List() {
	const [items, setItems] = createSignal([1, 2, 3]);
	const [activeItem, setActiveItem] = createSignal<number | null>(null);
	const ids = () => items();

	const onDragStart: DragEventHandler = ({ draggable }) => setActiveItem(draggable.id as number);

	const onDragEnd: DragEventHandler = ({ draggable, droppable }) => {
		if (draggable && droppable) {
			const currentItems = ids();
			const fromIndex = currentItems.indexOf(draggable.id as number);
			const toIndex = currentItems.indexOf(droppable.id as number);
			if (fromIndex !== toIndex) {
				const updatedItems = currentItems.slice();
				updatedItems.splice(toIndex, 0, ...updatedItems.splice(fromIndex, 1));
				setItems(updatedItems);
			}
		}
	};

	return (
		<DragDropProvider
			onDragStart={onDragStart}
			onDragEnd={onDragEnd}
			collisionDetector={closestCenter}
		>
			<DragDropSensors />
			<div class="column self-stretch">
				<SortableProvider ids={ids()}>
					<For each={items()}>{item => <Item item={item} />}</For>
				</SortableProvider>
			</div>
			<DragOverlay>
				<div class="sortable">{activeItem()}</div>
			</DragOverlay>
		</DragDropProvider>
	);
}

function BrowserPage() {
	return (
		<div class="flex flex-col gap-y-4">
			<h1>Browser</h1>
			<List />
		</div>
	);
}

export default BrowserPage;
