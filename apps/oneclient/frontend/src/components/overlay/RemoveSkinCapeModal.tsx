import { Overlay } from '@/components';
import { Button } from '@onelauncher/common/components';

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
