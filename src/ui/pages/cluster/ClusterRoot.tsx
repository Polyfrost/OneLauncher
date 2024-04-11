import { useSearchParams } from '@solidjs/router';
import type { ParentProps } from 'solid-js';
import { EyeIcon, File06Icon, PackagePlusIcon } from '@untitled-theme/icons-solid';
import Sidebar from '../../components/Sidebar';
import AnimatedRoutes from '../../components/AnimatedRoutes';
import ErrorBoundary from '../../components/ErrorBoundary';

function ClusterRoot(props: ParentProps) {
	const [searchParams] = useSearchParams();

	return (
		<div class="flex flex-row flex-1 h-full gap-x-7">
			<div class="mt-8">
				<Sidebar
					base="/clusters"
					state={{ id: searchParams.id }}
					links={{
						Cluster: [
							[<EyeIcon />, 'Overview', '/'],
							[<PackagePlusIcon />, 'Mods', '/mods'],
							[<File06Icon />, 'Logs', '/logs'],
						],
					}}
				/>
			</div>

			<div class="flex flex-col w-full h-full">
				<AnimatedRoutes>
					{/* <ErrorBoundary> */}
						{props.children}
					{/* </ErrorBoundary> */}
				</AnimatedRoutes>
			</div>
		</div>
	);
}

export default ClusterRoot;
