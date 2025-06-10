import { MyContextProvider } from '@/components/MyContext';
import { createFileRoute, Outlet } from '@tanstack/react-router';

export const Route = createFileRoute('/test')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<MyContextProvider value={{ text: 'Hello from cool!' }}>
			<div>Hello "/test"!</div>
			<Outlet />
		</MyContextProvider>
	);
}
