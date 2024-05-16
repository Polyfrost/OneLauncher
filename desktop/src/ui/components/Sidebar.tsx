import { useLocation, useNavigate } from '@solidjs/router';
import type { JSX, ParentProps } from 'solid-js';
import { For, createEffect } from 'solid-js';

type SidebarProps = ParentProps & {
	state: {
		[key: string]: any;
	};
	base: string;
	links: {
		[key: string]: [JSX.Element, string, string][];
	};
};

function Sidebar(props: SidebarProps) {
	const searchParams = new URLSearchParams();
	const navigate = useNavigate();
	const location = useLocation();

	createEffect(() => {
		if (props.base.endsWith('/'))
			throw new Error('Base should not end with a slash');

		Object.keys(props.state).forEach((key) => {
			searchParams.set(key, props.state[key]);
		});
	});

	function goto(href: string) {
		const url = `${props.base}${href}?${searchParams.toString()}`;
		navigate(url);
	}

	function isActive(link: string) {
		return location.pathname === `${props.base}${link}` || `${location.pathname}/` === `${props.base}${link}`;
	}

	return (
		<div class="flex flex-col w-52">
			<For each={Object.keys(props.links)}>
				{section => (
					<div class="flex flex-col gap-y-2">
						<div>
							<h3 class="text-fg-secondary text-xs font-medium m-1.5 mt-5">{section.toUpperCase()}</h3>
							<div class="flex flex-col gap-y-1 fill-fg-primary text-fg-primary font-medium">
								<For each={props.links[section]}>
									{link => (
										<a
											onClick={() => goto(link[2])}
											class={
                                                `px-3 py-1 rounded-md text-md hover:bg-component-bg-hover active:bg-component-bg-pressed ${isActive(link[2]) ? 'bg-gray-05' : ''}`
                                            }
										>
											<span class="flex flex-row items-center gap-x-3 *:w-5">
												{link[0]}
												{link[1]}
											</span>
										</a>
									)}
								</For>
							</div>
						</div>
					</div>
				)}
			</For>
		</div>
	);
}

export default Sidebar;
