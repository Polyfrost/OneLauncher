import ScrollableContainer from '~ui/components/ScrollableContainer';
import Sidebar from '~ui/components/Sidebar';
import useClusterContext from '~ui/hooks/useCluster';

function ClusterGame() {
	const cluster = useClusterContext();

	// TODO: Save cluster settings

	return (
		<Sidebar.Page>
			<h1>Game Running</h1>
			<ScrollableContainer>
				<p>TODO</p>
			</ScrollableContainer>
		</Sidebar.Page>
	);
}

export default ClusterGame;
