import { Button } from '@onelauncher/common/components';
import { Overlay } from './Overlay';

export function RemoveSkinCapeModal({ onPress }: { onPress: () => void }) {
	return (
		<Overlay.Dialog>
			<Overlay.Title>Are you sure?</Overlay.Title>

			<p>This cannot be undone</p>

			<Button
				className="w-full"
				color="danger"
				onPress={onPress}
				size="large"
				slot="close"
			>
				Remove
			</Button>
		</Overlay.Dialog>
	);
}
