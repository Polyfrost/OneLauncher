import type { ParentProps } from 'solid-js';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';

function ScrollableContainer(props: ParentProps) {
	return (
		<div class="h-full h-screen w-[calc(100%+14px)] flex flex-1 flex-col overflow-hidden">
			<OverlayScrollbarsComponent class="os-hide-horizontal-scrollbar relative flex-1">
				<div class="absolute h-full w-[calc(100%-14px)] flex flex-col gap-2">
					{props.children}
				</div>
			</OverlayScrollbarsComponent>
		</div>
	);
}

export default ScrollableContainer;
