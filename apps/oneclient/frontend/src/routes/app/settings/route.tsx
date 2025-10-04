import type { JSX } from 'react';
import { SheetPage } from '@/components';
import { createFileRoute, Outlet, useNavigate, useRouterState } from '@tanstack/react-router';
import { Brush01Icon, CodeSnippet02Icon, MessageTextSquare01Icon, RefreshCcw02Icon, Rocket02Icon, Sliders04Icon } from '@untitled-theme/icons-react';
import { useEffect } from 'react';

export const Route = createFileRoute('/app/settings')({
	component: RouteComponent,
});

function HeaderLarge() {
	return (
		<div className="flex flex-row justify-between items-end gap-8">
			<div className="flex flex-col">
				<h1 className="text-3xl font-semibold">Settings</h1>
				<p className="text-md font-medium text-fg-secondary">Adjust the Launcher Settings</p>
			</div>
		</div>
	);
}

function HeaderSmall() {
	return <h1 className="text-2lg h-full font-medium">Settings</h1>;
}

function RouteComponent() {
	return (
		<SheetPage headerLarge={<HeaderLarge />} headerSmall={<HeaderSmall />}>

			<div className="h-full flex flex-1 flex-row gap-x-7">
				<div className="mt-8 flex flex-col justify-between">
					<Sidebar
						base="/app/settings"
						exact
						links={{
							'Launcher Settings': [
								[<Rocket02Icon key="general" />, 'General', '/'],
								[<Brush01Icon key="appearence" />, 'Appearance', '/appearance'],
							// [<Key01Icon />, 'APIs', '/apis'],
							// [<Globe01Icon />, 'Language', '/language'],
							],
							'Game Settings': [
								[<Sliders04Icon key="mcsettings" />, 'Minecraft settings', '/minecraft'],
							],
							'About': [
								[<RefreshCcw02Icon key="changelog" />, 'Changelog', '/changelog'],
								[<MessageTextSquare01Icon key="feedback" />, 'Feedback', '/feedback'],
								[<CodeSnippet02Icon key="dev" />, 'Developer Options', '/developer'],
							],
						}}

					/>
					{/* <Info /> */}
				</div>

				<div className="h-full w-full flex flex-col">
					<Outlet />
				</div>
			</div>
		</SheetPage>
	);
}

interface SidebarProps {
	base: string;
	links: Record<string, Array<[JSX.Element, string, string, URLSearchParams?] | undefined>>;
	exact?: boolean;
}

function Sidebar(props: SidebarProps) {
	const { base, links, exact } = props;

	const navigate = useNavigate();
	const routerState = useRouterState();

	const location = routerState.location.pathname;

	useEffect(() => {
		if (base.endsWith('/'))
			throw new Error('Base should not end with a slash');
	});

	function goto(href: string, params?: URLSearchParams) {
		const currParams = new URLSearchParams(location);
		if (params)
			for (const [key, value] of params)
				currParams.set(key, value);

		const url = `${base}${href}`;
		navigate({ to: url });
	}

	function isActive(link: string, params: URLSearchParams | undefined, exact: boolean = false) {
		const fullBasePath = `${base}${link}`;
		const pathnameMatch = exact
			? location === fullBasePath || `${location}/` === fullBasePath
			: location.startsWith(fullBasePath) || `${location}/`.startsWith(fullBasePath);

		if (!params)
			return pathnameMatch;

		const searchParams = new URLSearchParams(routerState.location.searchStr);
		for (const [key, value] of params)
			if (searchParams.get(key) !== value)
				return false;

		return pathnameMatch;
	}

	return (
		<div className="w-56 flex flex-col pr-2">
			{Object.keys(links).map(section => (
				<div className="flex flex-col gap-y-2" key={section}>
					<div>
						<p className="m-1.5 mt-5 text-xs text-fg-secondary font-medium">{section.toUpperCase()}</p>
						<div className="flex flex-col gap-y-1 fill-fg-primary text-fg-primary font-medium">
							{links[section].map((entry) => {
								if (!entry)
									return '';

								const [Component, label, link, params] = entry;

								return (
									<a
										className={
											`px-3 py-1 rounded-md text-md border hover:bg-component-bg-hover active:bg-component-bg-pressed ${isActive(link, params, exact) ? 'bg-component-bg border-component-bg-pressed' : 'border-transparent'}`
										}
										key={link}
										onClick={() => goto(link, params)}
									>
										<span className="flex flex-row items-center gap-x-3 *:w-5">
											{Component}
											{label}
										</span>
									</a>
								);
							})}
						</div>
					</div>
				</div>
			))}
		</div>
	);
}

interface SidebarPageProps {
	className?: string;
	children: JSX.Element | Array<JSX.Element>;
}

Sidebar.Page = function (props: SidebarPageProps) {
	return (
		<div {...props} className={`flex flex-col flex-1 gap-y-1 overflow-y-auto h-full ${props.className || ''}`}>
			{props.children}
		</div>
	);
};

export default Sidebar;
