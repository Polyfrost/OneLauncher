import { useSettings } from '@/hooks/useSettings';
import { createFileRoute, Navigate } from '@tanstack/react-router';

export const Route = createFileRoute('/')({
	component: RouteComponent,
});

function RouteComponent() {
	const { setting } = useSettings();
	if (setting('seen_onboarding') === false)
		return <Navigate to="/onboarding" />;

	return <Navigate to="/app" />;
}
