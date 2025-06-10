import useMyContext from '@/components/MyContext';
import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/test/app')({
	component: RouteComponent,
});

function RouteComponent() {
	const myContext = useMyContext();

	return (
		<div>
			<div>Hello "/test/app"!</div>
			<div>
				Value from MyContext:
				{' '}
				{myContext.text}
			</div>
		</div>
	);
}
