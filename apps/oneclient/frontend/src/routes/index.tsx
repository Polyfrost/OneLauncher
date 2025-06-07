import { bindings } from '@/main';
import { Button } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/')({
	component: App,
});

function App() {
	const onClick = async () => {
		// const result = await bindings.core.createCluster({
		// 	name: 'Test 1.8.9 Forge',
		// 	mc_loader: 'forge',
		// 	mc_version: '1.8.9',
		// 	icon: null,
		// 	mc_loader_version: '11.15.1.2318',
		// });

		await bindings.core.launchCluster(2 as unknown as bigint, null);

		const unlisten = await bindings.events.process.on((e) => {
			if (e.type === 'Output') { console.log(e.output); }
			else if (e.type === 'Stopped') {
				console.log(e.exit_code);
				unlisten();
			}
		});
	};

	return (
		<div className="text-center">
			<h1>test</h1>
			<Button onClick={onClick}>Click Me!</Button>
		</div>
	);
}
