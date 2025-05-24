import Button from '@/components/base/Button';
import { TextField } from '@/components/base/TextField';
import Modal from '@/components/overlay/Modal';
import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/app/browser/')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<div>
			<p>asdasdas</p>

			<TextField />

			{/* we can open the modal but we cant close it for some reason */}
			<Modal.Trigger>
				<Button>Open Modal</Button>

				<Modal>
					<p>sadsadsad</p>
				</Modal>
			</Modal.Trigger>
		</div>
	);
}
