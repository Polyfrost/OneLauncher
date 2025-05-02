import { getProgramInfo } from '@onelauncher/client';
import { type ParentProps, Show, createMemo } from 'solid-js';
import { Transition  } from 'solid-transition-group';
import type {TransitionProps} from 'solid-transition-group';
import useSettings from '~ui/hooks/useSettings';

type AnimationTypes = 'default' | 'fade';
export interface AnimatedProps {
	animation?: AnimationTypes;
};

const animations: Record<AnimationTypes, { before: Keyframe; after: Keyframe }> = {
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

	const durationMultiplier = createMemo(() => {
		// I LOVE WEBKITGTK I LOVE WEBKTIGTKI LOVE WEBKITGTK I LOVE WEBKTIGTKI LOVE WEBKITGTK I LOVE WEBKTIGTK
		if (getProgramInfo().platform === 'linux')
			return 0.36;

		return 1;
	});

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
							duration: 250 * durationMultiplier(),
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
							duration: 255 * durationMultiplier(),
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
