import { For } from 'solid-js';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Sidebar from '~ui/components/Sidebar';

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
		<Sidebar.Page>
			<h1>Screenshots</h1>
			<ScrollableContainer>
				<div class="grid grid-cols-[repeat(auto-fill,minmax(350px,1fr))] w-full gap-2">
					<For each={list}>
						{screenshot => (
							<ScreenshotEntry {...screenshot} />
						)}
					</For>
				</div>
			</ScrollableContainer>
		</Sidebar.Page>
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
