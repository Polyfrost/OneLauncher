import { createFileRoute } from '@tanstack/react-router';
import { BracketsEllipsesIcon } from '@untitled-theme/icons-react';

export const Route = createFileRoute('/app/browser/')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<div>
			<div className="flex flex-col items-center justify-center h-full">
				<BracketsEllipsesIcon className="size-40" />
				<h3 className="text-xl">WIP</h3>
			</div>
		</div>
	);
}
