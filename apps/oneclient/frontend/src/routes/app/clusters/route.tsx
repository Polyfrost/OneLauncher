import { SheetPage } from '@/components/SheetPage';
import { Button } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/app/clusters')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<SheetPage
			headerLarge={<HeaderLarge />}
			headerSmall={<HeaderSmall />}
		/>
	);
}

function HeaderLarge({ ref }: { ref?: React.Ref<HTMLDivElement> }) {
	return (
		<div className="flex flex-row justify-between items-end" ref={ref}>
			<div className="flex flex-col">
				<h1 className="text-3xl font-semibold">Header</h1>
				<p className="text-md font-medium text-fg-secondary">Tailored for Hypixel Skyblock.</p>
			</div>

			<Button color="primary" size="large">
				Launch
			</Button>
		</div>
	);
}

function HeaderSmall() {
	return (
		<div className="flex flex-row w-full items-center justify-between">
			<div className="flex flex-row items-center gap-4">
				<h1 className="text-2lg h-full font-medium">Header</h1>
			</div>

			<Button color="primary">
				Launch
			</Button>
		</div>
	);
}
