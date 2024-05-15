import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import { For } from 'solid-js';

const list = Array<Screenshot>(19).fill({
	date_taken: Math.floor(Date.now() / 1000),
	path: 'https://www.researchgate.net/publication/301648368/figure/fig2/AS:667842240315400@1536237406360/A-screenshot-from-Minecraft-a-popular-video-game-which-poses-a-challenging-lifelong.ppm',
});

// TODO: Make sure the screenshots in the list are downscaled, grid / container can get laggy on low end devices
interface Screenshot {
	path: string;
	date_taken: number;
};

function ClusterScreenshots() {
	return (
		<div class="flex flex-col flex-1">
			<h1>Screenshots</h1>

			<div class="flex flex-col flex-1 rounded-lg overflow-hidden w-[calc(100%+14px)]">
				<OverlayScrollbarsComponent class="os-hide-horizontal-scrollbar relative flex-1">
					<div class="grid grid-cols-[repeat(auto-fill,minmax(350px,1fr))] absolute w-[calc(100%-14px)] gap-2">
						<For each={list}>
							{screenshot => (
								<ScreenshotEntry {...screenshot} />
							)}
						</For>
					</div>
				</OverlayScrollbarsComponent>
			</div>
		</div>
	);
}

export default ClusterScreenshots;

function ScreenshotEntry(props: Screenshot) {
	return (
		<div class="bg-component-bg hover:bg-component-bg-hover hover:opacity-80 active:bg-component-bg-pressed p-3 gap-3 rounded-xl flex flex-col items-center">
			{/* <div> */}
			<img src={props.path} alt={props.path} class="aspect-ratio-video w-full rounded-lg" />
			{/* </div> */}
		</div>
	);
}
