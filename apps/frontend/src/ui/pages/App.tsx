import type { ParentProps } from 'solid-js';
import { useNavigate } from '@solidjs/router';
import useSettings from '~ui/hooks/useSettings';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import AnimatedRoutes from '../components/AnimatedRoutes';
import Navbar from '../components/Navbar';

function App(props: ParentProps) {
	const { settings } = useSettings();
	const navigate = useNavigate();

	if (settings().onboarding_completed !== true)
		navigate('/onboarding');

	return (
		<div class="h-full flex flex-col">
			<div class="flex flex-col px-8">
				<Navbar />
			</div>

			<div class="h-full w-full overflow-hidden">
				<div class="relative h-full w-full flex flex-col overflow-x-hidden">
					<OverlayScrollbarsComponent class="os-hide-horizontal-scrollbar absolute left-0 top-0 h-full w-full flex flex-col overflow-x-hidden overflow-y-auto px-8 pb-8">
						<div class="h-full flex-1">
							<AnimatedRoutes>
								{props.children}
							</AnimatedRoutes>
						</div>
					</OverlayScrollbarsComponent>
				</div>
			</div>
		</div>
	);
}

export default App;
