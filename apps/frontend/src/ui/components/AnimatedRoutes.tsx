import useSettings from '~ui/hooks/useSettings';
import { type ParentProps, Show } from 'solid-js';
import { Transition, type TransitionProps } from 'solid-transition-group';

type AnimationTypes = 'default' | 'fade';
export interface AnimatedProps {
	animation?: AnimationTypes;
};

const animations: { [key in AnimationTypes]: { before: Keyframe; after: Keyframe } } = {
	default: {
		before: {
			transform: 'translateX(-85px)',
			opacity: 0,
		},
		after: {
			transform: 'translateX(0)',
			opacity: 1,
		},
	},
	fade: {
		before: {
			opacity: 0,
		},
		after: {
			opacity: 1,
		},
	},
};

function AnimatedRoutes(props: AnimatedProps & TransitionProps & ParentProps) {
	const { settings } = useSettings();

	const animation = () => animations[props.animation ?? 'default'];

	return (
		<Show
			children={(
				<Transition
					mode="outin"
					onEnter={(el, done) => {
						if (settings().disable_animations === true) {
							done();
							return;
						}

						el.animate([
							animation().before,
							animation().after,
						], {
							duration: 90,
							easing: 'cubic-bezier(0.16, 1, 0.3, 1)',
						}).onfinish = () => {
							done();
						};
					}}
					onExit={(el, done) => {
						if (settings().disable_animations === true) {
							done();
							return;
						}

						el.animate([
							animation().after,
							animation().before,
						], {
							duration: 95,
							easing: 'cubic-bezier(0.16, 1, 0.3, 1)',
						}).onfinish = () => {
							done();
						};
					}}
				>
					{props.children}
				</Transition>
			)}
			fallback={(<>{props.children}</>)}
			when={settings().disable_animations !== true}
		/>
	);
}

export default AnimatedRoutes;
