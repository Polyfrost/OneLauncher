import type { ParentProps } from 'solid-js';
import { Transition } from 'solid-transition-group';
import environment from '../utils/environment';
import WindowFrame from './components/native/WindowFrame';
import Navbar from './components/Navbar';

function App(props: ParentProps) {
	if (!environment.isDev()) {
		document.addEventListener('contextmenu', (event) => {
			event.preventDefault();
		});
	}

	return (
		<main class="flex flex-col bg-primary w-full min-h-screen overflow-hidden h-screen max-h-screen text-white">

			<WindowFrame />
			<div class="flex flex-col px-8">
				<Navbar />
			</div>

			<div class="relative h-full w-full overflow-hidden">
				<div class="absolute top-0 left-0 flex flex-col h-full w-full overflow-x-hidden">
					<div class="absolute top-0 left-0 flex flex-col h-full w-full overflow-x-hidden overflow-y-auto px-8">
						<AnimatedRoutes>
							{props.children}
						</AnimatedRoutes>
					</div>
				</div>
			</div>
		</main>
	);
}

export default App;

function AnimatedRoutes(props: ParentProps) {
	const keyframesEnter = [
		{
			opacity: 0,
			transform: 'translateX(-100px)',
		},
		{
			opacity: 1,
			transform: 'translateX(0px)',
		},
	];

	const keyframesExit = [
		{
			opacity: 1,
			transform: 'translateX(0px)',
		},
		{
			opacity: 0,
			transform: 'translateX(100px)',
		},
	];

	const properties: KeyframeAnimationOptions = {
		duration: 100,
		easing: 'cubic-bezier(0.22, 1, 0.36, 1)',
	};

	return (
		<Transition
			mode="outin"
			onEnter={(element, done) => {
				const animation = element.animate(
					keyframesEnter,
					properties,
				);

				animation.onfinish = done;
			}}
			onExit={(element, done) => {
				const animation = element.animate(
					keyframesExit,
					properties,
				);

				animation.onfinish = done;
			}}
		>
			{props.children}
		</Transition>
	);
}
