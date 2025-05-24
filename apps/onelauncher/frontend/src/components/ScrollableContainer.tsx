import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';

interface ScrollableContainerProps {
	children: React.ReactNode;
}

function ScrollableContainer(props: ScrollableContainerProps) {
	return (
		<div className="h-full h-screen w-[calc(100%+14px)] flex flex-1 flex-col overflow-hidden">
			<OverlayScrollbarsComponent className="os-hide-horizontal-scrollbar relative flex-1">
				<div className="absolute h-full w-[calc(100%-14px)] flex flex-col gap-2">
					{props.children}
				</div>
			</OverlayScrollbarsComponent>
		</div>
	);
}

export default ScrollableContainer;
