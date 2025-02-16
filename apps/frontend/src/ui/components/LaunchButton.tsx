import type { Cluster } from '@onelauncher/client/bindings';
import type { ButtonProps } from './base/Button';
import { debounce } from '@solid-primitives/scheduled';
import { PlayIcon } from '@untitled-theme/icons-solid';
import { useLaunchCluster } from '~ui/hooks/useCluster';
import useProcessor from '~ui/hooks/useProcessor';
import useSettings from '~ui/hooks/useSettings';
import { Match, splitProps, Switch } from 'solid-js';
import Button from './base/Button';

function LaunchButton(props: ButtonProps & { cluster: Cluster; iconOnly?: boolean }) {
	const [split, rest] = splitProps(props, ['cluster']);
	const innerLaunch = useLaunchCluster(() => split.cluster?.uuid);
	// eslint-disable-next-line solid/reactivity -- a
	const { running: runningProcesses } = useProcessor(split.cluster);
	const { settings } = useSettings();

	const launch = (e: MouseEvent) => {
		e.stopImmediatePropagation();
		e.preventDefault();
		debounce(innerLaunch, 300)();
	};

	return (
		<Switch>
			<Match when={props.iconOnly === true}>
				<Button
					buttonStyle="iconSecondary"
					children={<PlayIcon class="h-4! w-4!" />}
					disabled={settings().allow_parallel_running_clusters !== true && (runningProcesses()?.length || 0) > 0}
					onClick={launch}
					{...rest}
				/>
			</Match>
			<Match when>
				<Button
					buttonStyle="primary"
					children="Launch"
					disabled={settings().allow_parallel_running_clusters !== true && (runningProcesses()?.length || 0) > 0}
					iconLeft={<PlayIcon />}
					onClick={launch}
					{...rest}
				/>
			</Match>
		</Switch>
	);
}

export default LaunchButton;
