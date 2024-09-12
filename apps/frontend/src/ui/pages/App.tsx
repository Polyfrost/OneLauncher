import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import type { ParentProps } from 'solid-js';
import AnimatedRoutes from '../components/AnimatedRoutes';
import ErrorBoundary from '../components/ErrorBoundary';
import Navbar from '../components/Navbar';

function App(props: ParentProps) {
	return (
		<>
			<div class="flex flex-col px-8">
				<Navbar />
			</div>

			<div class="relative h-full w-full overflow-hidden">
				<div class="absolute left-0 top-0 h-full w-full flex flex-col overflow-x-hidden">
					<ErrorBoundary>
						<OverlayScrollbarsComponent class="os-hide-horizontal-scrollbar absolute left-0 top-0 h-full w-full flex flex-col overflow-x-hidden overflow-y-auto px-8 pb-8">
							<div class="h-full flex-1">
								<AnimatedRoutes>
									{props.children}
								</AnimatedRoutes>
							</div>
						</OverlayScrollbarsComponent>
					</ErrorBoundary>
				</div>
			</div>
		</>
	);
}

export default App;
