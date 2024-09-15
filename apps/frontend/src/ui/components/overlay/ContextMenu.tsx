import { type Accessor, createEffect, createSignal, type JSX } from 'solid-js';
import Popup, { type PopupProps } from './Popup';

type ContextMenuProps = {
	pos: Accessor<{ x: number; y: number }>;
} & PopupProps;

function ContextMenu(props: ContextMenuProps) {
	const [pos, setPos] = createSignal({ x: 0, y: 0 });
	const [ref, setRef] = createSignal<HTMLDivElement>();

	createEffect(() => {
		if (ref() === undefined)
			return;

		const rect = ref()!.getBoundingClientRect();

		let x = props.pos().x;
		let y = props.pos().y;

		if (x + rect.width > window.innerWidth)
			x = window.innerWidth - rect.width - 10;

		if (y + rect.height > window.innerHeight)
			y = window.innerHeight - rect.height - 10;

		setPos({ x, y });
	});

	return (
		<Popup
			ref={(el) => {
				el.style.left = `${pos().x}px`;
				el.style.top = `${pos().y}px`;
			}}
			setVisible={props.setVisible}
			visible={props.visible}
		>
			<div class="border border-gray-05 rounded-xl bg-page-elevated p-1 shadow-black/30 shadow-md" ref={setRef}>
				<div class="flex flex-col gap-y-1 text-fg-primary">
					{props.children}
				</div>
			</div>
		</Popup>
	);
}

ContextMenu.Seperator = function () {
	return <div class="mx-1 border-b border-gray-05" />;
};

interface ContextMenuRowProps {
	icon: JSX.Element;
	text: string;
	onClick?: (e: MouseEvent) => void;
}

ContextMenu.Row = function (props: ContextMenuRowProps) {
	return (
		<div
			class="m-px flex items-center gap-x-2 rounded-lg px-1.5 py-0.5 [&>svg]:w-[18px] active:bg-gray-10 hover:bg-gray-05"
			onClick={e => props.onClick?.(e)}
		>
			{props.icon}
			{props.text}
		</div>
	);
};

export default ContextMenu;
