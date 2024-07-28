import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import type { ParentProps } from 'solid-js';

function ScrollableContainer(props: ParentProps) {
	return (
		<div class="flex flex-col flex-1 overflow-hidden w-[calc(100%+14px)]">
			<OverlayScrollbarsComponent class="os-hide-horizontal-scrollbar relative flex-1">
				<div class="flex flex-col absolute w-[calc(100%-14px)] h-full gap-1">
					{props.children}
				</div>
			</OverlayScrollbarsComponent>
		</div>
	);
}

export default ScrollableContainer;
