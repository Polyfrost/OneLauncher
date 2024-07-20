import { type ParentProps, mergeProps, splitProps } from 'solid-js';
import { Transition, type TransitionProps } from 'solid-transition-group';

function AnimatedRoutes(props: TransitionProps & ParentProps) {
	const defaultProps: TransitionProps = {
		mode: 'outin',
		enterClass: 'page-animation-enter',
		enterActiveClass: 'page-animation-enter-active',
		enterToClass: 'page-animation-enter-to',
		exitClass: 'page-animation-leave',
		exitActiveClass: 'page-animation-leave-active',
		exitToClass: 'page-animation-leave-to',
	};

	const [split, rest] = splitProps(props, ['children']);
	const merged = mergeProps(defaultProps, rest);

	return (
		<Transition {...merged}>
			{split.children}
		</Transition>
	);
}

export default AnimatedRoutes;
