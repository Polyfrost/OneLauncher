import { Button } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/')({
	component: App,
});

function App() {
	return (
		<div className="text-center">
			<h1>test</h1>
			<Button>Click Me!</Button>
		</div>
	);
}
