import { createFileRoute, Navigate } from '@tanstack/react-router';

export const Route = createFileRoute('/')({
	component: App,
});

function App() {
	// TODO: This will decide if we need to show onboarding or not.
	return (
		<Navigate to="/app" />
	);
}
