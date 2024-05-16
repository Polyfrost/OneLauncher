import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import type { ParentProps } from 'solid-js';

type ScrollableContainerProps = ParentProps & {
	title: string;
};

function ScrollableContainer(props: ScrollableContainerProps) {
	return (
		<div class="flex flex-col flex-1">
			<h1>{props.title}</h1>
			<div class="flex flex-col flex-1 rounded-lg overflow-hidden w-[calc(100%+14px)]">
				<OverlayScrollbarsComponent class="os-hide-horizontal-scrollbar relative flex-1">
					<div class="flex flex-col absolute w-[calc(100%-14px)] gap-2">
						{props.children}
					</div>
				</OverlayScrollbarsComponent>
			</div>
		</div>
	);
}

export default ScrollableContainer;
