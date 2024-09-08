import { useLocation, useNavigate } from '@solidjs/router';
import { createEffect, For, splitProps } from 'solid-js';
import type { JSX, ParentProps } from 'solid-js';

type SidebarProps = ParentProps & {
	base: string;
	links: Record<string, ([JSX.Element, string, string, URLSearchParams?] | undefined)[]>;
};

function Sidebar(props: SidebarProps) {
	const navigate = useNavigate();
	const location = useLocation();

	createEffect(() => {
		if (props.base.endsWith('/'))
			throw new Error('Base should not end with a slash');
	});

	function goto(href: string, params?: URLSearchParams) {
		const currParams = new URLSearchParams(location.search);
		if (params)
			for (const [key, value] of params)
				currParams.set(key, value);

		const url = `${props.base}${href}?${currParams.toString()}`;
		navigate(url);
	}

	function isActive(link: string, params: URLSearchParams | undefined) {
		const pathnameMatch = location.pathname === `${props.base}${link}` || `${location.pathname}/` === `${props.base}${link}`;
		const paramsMatch = params ? location.search.endsWith(params.toString()) : true;
		return pathnameMatch && paramsMatch;
	}

	return (
		<div class="w-56 flex flex-col pr-2">
			<For each={Object.keys(props.links)}>
				{section => (
					<div class="flex flex-col gap-y-2">
						<div>
							<h3 class="m-1.5 mt-5 text-xs text-fg-secondary font-medium">{section.toUpperCase()}</h3>
							<div class="flex flex-col gap-y-1 fill-fg-primary text-fg-primary font-medium">
								<For each={props.links[section]}>
									{(link) => {
										if (!link)
											return;

										return (
											<a
												class={
													`px-3 py-1 rounded-md text-md border hover:bg-component-bg-hover active:bg-component-bg-pressed ${isActive(link[2], link[3]) ? 'bg-component-bg border-gray-05' : 'border-transparent'}`
												}
												onClick={() => goto(link[2], link[3])}
											>
												<span class="flex flex-row items-center gap-x-3 *:w-5">
													{link[0]}
													{link[1]}
												</span>
											</a>
										);
									}}
								</For>
							</div>
						</div>
					</div>
				)}
			</For>
		</div>
	);
}

Sidebar.Page = function (props: JSX.HTMLAttributes<HTMLDivElement>) {
	const [split, rest] = splitProps(props, ['class', 'children']);
	return (
		<div class={`flex flex-col flex-1 gap-y-1 ${split.class || ''}`} {...rest}>
			{split.children}
		</div>
	);
};

export default Sidebar;
