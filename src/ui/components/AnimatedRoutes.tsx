import type { ParentProps } from 'solid-js';
import { Transition } from 'solid-transition-group';

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

export default AnimatedRoutes;
