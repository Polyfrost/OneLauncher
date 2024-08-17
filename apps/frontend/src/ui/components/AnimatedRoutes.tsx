import { type ParentProps, Show } from 'solid-js';
import { Transition, type TransitionProps } from 'solid-transition-group';
import useSettingsContext from '~ui/hooks/useSettings';

function AnimatedRoutes(props: TransitionProps & ParentProps) {
	const { settings } = useSettingsContext();

	const before: Keyframe = {
		transform: 'translateX(-85px)',
		opacity: 0,
	};

	const after: Keyframe = {
		transform: 'translateX(0)',
		opacity: 1,
	};

	return (
		<Show
			when={settings().disable_animations !== true}
			fallback={(
				<>{props.children}</>
			)}
			children={(
				<Transition
					mode="outin"
					onEnter={(el, done) => {
						if (settings().disable_animations === true) {
							done();
							return;
						}

						el.animate([
							before,
							after,
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
							after,
							before,
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
		/>
	);
}

export default AnimatedRoutes;
