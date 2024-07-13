import type {
	Accessor,
	ParentProps,
	Setter,
} from 'solid-js';
import { Portal } from 'solid-js/web';

type FullscreenOverlayProps = {
	visible: Accessor<boolean>;
	setVisible: Setter<boolean>;
} & ParentProps;

function FullscreenOverlay(props: FullscreenOverlayProps) {
	function onBackdropClick() {
		props.setVisible(false);
	}

	return (
		<Portal>
			<div class={`fixed z-[1000] top-0 left-0 w-screen h-screen bg-black/60 backdrop-blur-sm backdrop-grayscale transition-opacity ${props.visible() ? 'opacity-100 pointer-events-auto' : 'opacity-0 pointer-events-none'}`}>
				<div class="absolute w-full h-full" onClick={() => onBackdropClick()} />

				<div class="absolute top-0 left-0 w-screen h-screen">
					{props.children}
				</div>

			</div>
		</Portal>
	);
}

export default FullscreenOverlay;
