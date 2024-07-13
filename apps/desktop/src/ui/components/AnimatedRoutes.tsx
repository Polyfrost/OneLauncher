import type { ParentProps } from 'solid-js';
import { Transition } from 'solid-transition-group';

function AnimatedRoutes(props: ParentProps) {
	return (
		<Transition
			mode="outin"
			enterClass="page-animation-enter"
			enterActiveClass="page-animation-enter-active"
			enterToClass="page-animation-enter-to"
			exitClass="page-animation-leave"
			exitActiveClass="page-animation-leave-active"
			exitToClass="page-animation-leave-to"
		>
			{props.children}
		</Transition>
	);
}

export default AnimatedRoutes;
