import type {
	Accessor,
	ParentProps,
	Setter,
} from 'solid-js';
import { Portal } from 'solid-js/web';

type FullscreenOverlayProps = {
	visible: Accessor<boolean>;
	setVisible: Setter<boolean>;
	mount?: Node | undefined;
	zIndex?: number;
} & ParentProps;

function FullscreenOverlay(props: FullscreenOverlayProps) {
	function onBackdropClick(this: HTMLDivElement, e: MouseEvent) {
		e.preventDefault();
		if (e.target === this)
			props.setVisible(false);
	}

	return (
		<Portal {...(props.mount ? { mount: props.mount } : {})}>
			<div
				style={{ 'z-index': props.zIndex || 1000 }}
				class={`fixed top-0 left-0 w-screen h-screen bg-black/60 backdrop-blur-sm backdrop-grayscale transition-opacity ${props.visible() ? 'opacity-100 pointer-events-auto' : 'opacity-0 pointer-events-none'}`}
			>
				{/* New element to prevent from having to call stopPropagation in child elements */}
				{/* <div class="absolute w-full h-full" onClick={() => onBackdropClick()} /> */}

				<div class="absolute top-0 left-0 w-screen h-screen" onClick={onBackdropClick}>
					{props.children}
				</div>

			</div>
		</Portal>
	);
}

export default FullscreenOverlay;
