import type { LauncherError } from '@/bindings.gen';
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

		try {
			// await bindings.core.launchCluster(9 as unknown as bigint, null);
			await bindings.core.removeCluster(9 as unknown as bigint);

			const unlisten = await bindings.events.process.on((e) => {
				if (e.type === 'Output') { console.log(e.output); }
				else if (e.type === 'Stopped') {
					console.log(e.exit_code);
					unlisten();
				}
			});
		}
		catch (err: unknown) {
			let error = err as LauncherError;
			// console.log(error);
			// if (error.type === 'DaoError')
			// 	if (error.data.type === 'NotFound')
			// 		console.log(error.data.data);
			if (error.type === 'TauriError')
				console.error(error.data);
			else if (error.type === 'IOError')
				console.error(error.data.data);
			else if (error.type === 'DaoError')
				console.error(error.data);

			console.log(error);
		}
	};

	return (
		<div className="text-center">
			<h1>test</h1>
			<Button onClick={onClick}>Click Me!</Button>
		</div>
	);
}
