import { type Accessor, type JSX, createEffect, createSignal } from 'solid-js';
import Popup from './Popup';

type ContextMenuProps = {
	pos: Accessor<{ x: number; y: number }>;
} & Popup.PopupProps;

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
			style={{ top: `${pos().y}px`, left: `${pos().x}px` }}
			visible={props.visible}
			setVisible={props.setVisible}
		>
			<div ref={setRef} class="bg-secondary rounded-xl border border-gray-0.10 p-1 shadow-lg shadow-black/50">
				<div class="flex flex-col gap-y-1 text-fg-primary">
					{props.children}
				</div>
			</div>
		</Popup>
	);
}

ContextMenu.Seperator = function () {
	return <div class="border-b mx-1 border-gray-0.05" />;
};

interface ContextMenuRowProps {
	icon: JSX.Element;
	text: string;
	onClick?: (e: MouseEvent) => any;
}

ContextMenu.Row = function (props: ContextMenuRowProps) {
	return (
		<div
			onClick={e => props.onClick?.(e)}
			class="flex items-center gap-x-2 m-px px-1.5 py-0.5 rounded-mdlg hover:bg-gray-0.05 active:bg-gray-0.10 [&>svg]:w-[18px]"
		>
			{props.icon}
			{props.text}
		</div>
	);
};

export default ContextMenu;
