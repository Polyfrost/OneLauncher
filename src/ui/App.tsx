import type { ParentProps } from 'solid-js';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import environment from '../utils/environment';
import WindowFrame from './components/WindowFrame';
import Navbar from './components/Navbar';
import AnimatedRoutes from './components/AnimatedRoutes';

function App(props: ParentProps) {
	if (!environment.isDev()) {
		document.addEventListener('contextmenu', (event) => {
			event.preventDefault();
		});
	}

	return (
		<main class="flex flex-col bg-primary w-full min-h-screen overflow-hidden h-screen max-h-screen text-fg-primary">
			<WindowFrame />
			<div class="flex flex-col px-8">
				<Navbar />
			</div>

			{/* This was a pain to do */}
			<div class="relative h-full w-full overflow-hidden">
				<div class="absolute top-0 left-0 flex flex-col h-full w-full overflow-x-hidden">
					<OverlayScrollbarsComponent class="absolute top-0 left-0 flex flex-col h-full w-full overflow-x-hidden overflow-y-auto px-8 pb-8">
						<AnimatedRoutes>
							{props.children}
						</AnimatedRoutes>
					</OverlayScrollbarsComponent>
				</div>
			</div>

		</main>
	);
}

export default App;
