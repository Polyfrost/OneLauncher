import ScrollableContainer from '~ui/components/ScrollableContainer';
import Sidebar from '~ui/components/Sidebar';
import useClusterContext from '~ui/hooks/useCluster';
import useSettingsContext from '~ui/hooks/useSettings';
import SettingsMinecraft from '~ui/pages/settings/game/SettingsMinecraft';

function ClusterSettings() {
	const [cluster] = useClusterContext();
	const settings = useSettingsContext();

	// TODO: Save cluster settings

	return (
		<Sidebar.Page>
			<h1>Overview</h1>
			<ScrollableContainer>
				<SettingsMinecraft.Settings
					fullscreen={{
						get: cluster()?.force_fullscreen ?? settings.force_fullscreen ?? false,
						set: value => cluster() && (cluster()!.force_fullscreen = value),
						isGlobal: cluster()?.force_fullscreen === null,
					}}
					resolution={{
						get: cluster()?.resolution ?? settings.resolution,
						set: value => cluster() && (cluster()!.resolution = value),
						isGlobal: cluster()?.resolution === undefined || cluster()?.resolution === null,
					}}
				/>
			</ScrollableContainer>
		</Sidebar.Page>
	);
}

export default ClusterSettings;
