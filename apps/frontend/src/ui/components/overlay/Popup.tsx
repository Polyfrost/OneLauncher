import { mergeRefs } from '@solid-primitives/refs';
import { createEffect } from 'solid-js';
import type {
	Accessor,
	JSX,
	ParentProps,
	Setter,
} from 'solid-js';
import { Portal } from 'solid-js/web';

export type PopupProps = Omit<Parameters<typeof Portal>[0], 'children'> & ParentProps & {
	visible: Accessor<boolean>;
	setVisible: Setter<boolean>;
	class?: string;
	style?: JSX.CSSProperties | string;
	mount?: Node;
};

function Popup(props: PopupProps) {
	let popupRef!: HTMLDivElement;

	function onMouseDown(e: MouseEvent) {
		e.stopPropagation();
		if (!popupRef || !props.visible())
			return;

		const clicked = e.target === popupRef || popupRef.contains(e.target as Node);
		if (!clicked)
			props.setVisible(false);
	}

	createEffect(() => {
		if (props.visible())
			document.addEventListener('mousedown', onMouseDown);
		else
			document.removeEventListener('mousedown', onMouseDown);
	});

	return (
		<Portal
			mount={props.mount || document.body}
			ref={mergeRefs((el) => {
				el.classList.add('absolute', 'z-[1000]', 'pointer-events-none');
				return el;
			}, props.ref)}
		>
			<div ref={popupRef} style={props.style || ''} class={`transition-opacity ${props.visible() ? 'opacity-100 pointer-events-auto' : 'opacity-0 pointer-events-none'} ${props.class || ''}`}>
				{props.children}
			</div>
		</Portal>
	);
}

Popup.setPos = (parent: HTMLElement, ref: (SVGGElement | HTMLDivElement)) => {
	const parentRect = parent.getBoundingClientRect();

	const top = parentRect.bottom;
	const right = document.body.clientWidth - parentRect.right;

	ref.style.top = `${top}px`;
	ref.style.right = `${right}px`;
};

export default Popup;
