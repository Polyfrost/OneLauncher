import type { OverlayProps } from './Overlay';
import { Overlay } from './Overlay';

export interface PolyBundleModalProps extends OverlayProps {

}

export function PolyBundleModal() {
	return (
		<Overlay.Dialog>
			<Overlay.Title>test</Overlay.Title>
		</Overlay.Dialog>
	);
}
