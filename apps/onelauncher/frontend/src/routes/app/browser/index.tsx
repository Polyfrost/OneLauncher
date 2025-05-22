import { TextField } from '@/components/base/TextField';
import { Modal } from '@/components/overlay/Modal';
import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/app/browser/')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<div>
			<p>asdasdas</p>

			<TextField />

			<Modal />
		</div>
	);
}
