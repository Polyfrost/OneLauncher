import { GameBackground } from '@/components';
import { SheetPage } from '@/components/SheetPage';
import { Button } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { ArrowRightIcon } from '@untitled-theme/icons-react';

export const Route = createFileRoute('/app/clusters/')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<SheetPage
			headerLarge={<HeaderLarge />}
			headerSmall={<HeaderSmall />}
		>
			<div className="relative flex flex-row gap-4">
				<div className="flex flex-col flex-1">

					<div className="grid grid-cols-2 gap-4">
						<div className="rounded-xl overflow-hidden aspect-video brightness-50 inset-2 inset-ring inset-red-500">
							<GameBackground className="w-full h-full -z-10 relative" name="CavesAndCliffs" />
						</div>

						<div className="aspect-video">
							<GameBackground className="rounded-xl w-full h-full border-2 border-ghost-overlay" name="HypixelSkyblockHub" />
						</div>
					</div>

					<span>Hello World!</span>
					<div className="h-screen w-px">

					</div>
					<span>test</span>
				</div>

				<SheetPage.Content className="sticky top-8 w-86 h-full flex flex-col p-2 gap-2 border border-ghost-overlay">
					<GameBackground className="aspect-video w-full rounded-xl border-2 border-ghost-overlay" name="CavesAndCliffs" />

					<div className="flex flex-col px-4 pt-2 pb-4 gap-2">
						<h2 className="text-xxl font-medium">Version 1.21</h2>
						<p className="text-sm text-fg-secondary">
							Minecraft's 1.21 update, known as "Tricky Trials," primarily focuses on combat adventures and tinkering, introducing trial chambers, new copper block variants, a new crafting tool, and a new weapon. It also features new hostile mobs, paintings, and gameplay enhancements.
						</p>

						<div className="flex flex-row items-center justify-between mt-3">
							<p>Minor Version</p>
							<Button className="w-26" color="secondary">1.21.5</Button>
						</div>

						<div className="flex flex-row items-center justify-between mb-4">
							<p>Mod Loader</p>
							<Button className="w-26" color="secondary">Forge</Button>
						</div>

						<div className="w-full flex flex-row gap-4">
							<Button className="flex-1" size="large">Launch</Button>
							<Button className="flex-1" color="secondary" size="large">
								View
								<ArrowRightIcon />
							</Button>
						</div>

					</div>
				</SheetPage.Content>
			</div>
		</SheetPage>
	);
}

function HeaderLarge({ ref }: { ref?: React.Ref<HTMLDivElement> }) {
	return (
		<div className="flex flex-row justify-between items-end gap-8" ref={ref}>
			<div className="flex flex-col">
				<h1 className="text-3xl font-semibold">Clusters</h1>
				<p className="text-md font-medium text-fg-secondary">Something something in corporate style fashion about picking your preferred gamemodes and versions and optionally loader so that oneclient can pick something for them</p>
			</div>
		</div>
	);
}

function HeaderSmall() {
	return (
		<h1 className="text-2lg h-full font-medium">Clusters</h1>
	);
}
