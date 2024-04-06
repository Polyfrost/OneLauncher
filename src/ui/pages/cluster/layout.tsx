import { useSearchParams } from '@solidjs/router';
import type { ParentProps } from 'solid-js';
import { EyeIcon, PackagePlusIcon } from '@untitled-theme/icons-solid';
import Sidebar from '../../components/Sidebar';
import AnimatedRoutes from '../../components/AnimatedRoutes';

function ClusterPage(props: ParentProps) {
	const [searchParams] = useSearchParams();

	return (
		<div class="flex flex-row gap-x-7">
			<div class="mt-8">
				<Sidebar
					base="/clusters"
					state={{ id: searchParams.id }}
					links={{
						Cluster: [
							[<EyeIcon />, 'Overview', '/'],
							[<PackagePlusIcon />, 'Mods', '/mods'],
						],
					}}
				/>
			</div>

			<div>
				<AnimatedRoutes>
					{props.children}
				</AnimatedRoutes>
			</div>
		</div>
	);
}

export default ClusterPage;
