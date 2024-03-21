import type {
	Accessor,
	ParentProps,
	Setter,
} from 'solid-js';
import {
	Show,
	createEffect,
	createSignal,
} from 'solid-js';
import { Portal } from 'solid-js/web';

// eslint-disable-next-line ts/no-namespace
declare namespace Popup {
	interface PopupProps extends ParentProps {
		visible: Accessor<boolean>;
		setVisible: Setter<boolean>;
		class?: string;
		mount?: Node;
	}
}

function Popup(props: Popup.PopupProps) {
	const [localVisible, setLocalVisible] = createSignal(false);
	const [animate, setAnimate] = createSignal('animate-fade-in');

	let popupRef!: HTMLDivElement;

	function onMouseDown(e: MouseEvent) {
		e.stopPropagation();
		if (!popupRef || !localVisible())
			return;

		const clicked = e.target === popupRef || popupRef.contains(e.target as Node);
		if (!clicked)
			props.setVisible(false);
	}

	createEffect(() => {
		if (props.visible()) {
			document.addEventListener('mousedown', onMouseDown);
			setAnimate('animate-fade-in');
			setLocalVisible(true);
		}
		else {
			document.removeEventListener('mousedown', onMouseDown);
			setAnimate('animate-fade-out');
			setTimeout(() => {
				setLocalVisible(false);
			}, 150);
		}
	});

	return (
		<Show when={localVisible()}>
			<Portal mount={props.mount || document.body}>
				<div ref={popupRef} class={`absolute z-[1000] ${animate()} ${props.class || ''}`}>
					{props.children}
				</div>
			</Portal>
		</Show>
	);
}

export default Popup;
