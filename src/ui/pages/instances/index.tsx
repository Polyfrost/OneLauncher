import { A, type AnchorProps, useNavigate, useSearchParams } from '@solidjs/router';
import { type ParentProps, createEffect, splitProps } from 'solid-js';

function Link(props: AnchorProps) {
	const [split, rest] = splitProps(props, ['href']);
	const [searchParams] = useSearchParams();

	return (
		<A {...rest} href={`${split.href}?id=${searchParams.id}`} />
	);
}

function InstancePage(props: ParentProps) {
	const [searchParams] = useSearchParams();
	const navigate = useNavigate();

	createEffect(() => {
		if (typeof searchParams.id !== 'string')
			navigate('/');
	});

	return (
		<div>
			<div>
				<Link href="/instances/">Overview</Link>
				<Link href="/instances/mods">mods</Link>
			</div>
			{props.children}
			{searchParams.id}
		</div>
	);
}

export default InstancePage;
