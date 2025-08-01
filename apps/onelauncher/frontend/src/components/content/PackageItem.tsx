import type { Provider, SearchResult } from '@/bindings.gen';
import { abbreviateNumber, LOADERS, upperFirst } from '@/utils';
import { Show, Tooltip } from '@onelauncher/common/components';
import { Link } from '@tanstack/react-router';
import { Download01Icon } from '@untitled-theme/icons-react';
import { useMemo } from 'react';
import { Focusable } from 'react-aria-components';
import { FlatLoaderIcon } from '../launcher/FlatLoaderIcon';

function includes<T, TArray extends T>(list: { includes: (arg0: TArray) => boolean }, element: T): element is TArray {
	return list.includes(element as unknown as TArray);
}

export function PackageGrid({ items, provider }: { items: Array<SearchResult>; provider: Provider }) {
	return (
		<div className="grid grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4 min-[1900px]:grid-cols-5! gap-2">
			{items.map(item => (
				<PackageItem key={item.project_id} {...item} provider={provider} />
			))}
		</div>
	);
}

export function PackageItem({ provider, ...item }: SearchResult & { provider: Provider }) {
	const loaders = useMemo(() => item.categories.filter(cat => includes(LOADERS, cat)), [item.categories]);

	return (
		<Link
			className="h-full min-w-50 flex overflow-hidden rounded-lg bg-component-bg hover:bg-component-bg-hover flex-col max-h-74 min-h-74"
			params={{ provider, slug: item.slug }}
			tabIndex={0}
			to="/app/browser/package/$provider/$slug"
		>
			<div
				className="relative flex items-center justify-center overflow-hidden w-full h-28"
			>
				<Show
					fallback={(
						<div
							className="aspect-ratio-square rounded-md bg-border/05 w-2/5"
						/>
					)}
					when={item.icon_url}
				>
					<img alt={`Icon for ${item.title}`} className="absolute z-0 max-w-none w-7/6 opacity-50 blur-xl" src={item.icon_url} />
					<img
						alt={`Icon for ${item.title}`}
						className="relative z-1 aspect-ratio-square rounded-lg image-render-auto h-5/6"
						src={item.icon_url}
					/>
				</Show>
				<Tooltip className="bg-component-bg-disabled" text={loaders.map(upperFirst).join(', ')}>
					<Focusable>
						<div className="flex flex-col rounded-full bg-[#11171C]/75 border-component-border/70 border p-1 absolute top-0 right-0 m-2">
							{loaders.toSpliced(loaders.length > 3 ? 2 : 3).map(loader => <FlatLoaderIcon className="w-4 m-1" key={loader} loader={loader} />)}
							{loaders.length > 3 && (
								<div className="bg-component-bg/50 rounded-full w-6 h-6 flex items-center justify-center">
									<span className="tracking-tight -ml-0.5 mt-0.5">
										+
										{loaders.length - 2}
									</span>
								</div>
							)}
						</div>
					</Focusable>
				</Tooltip>
			</div>
			<div className="flex flex-1 flex-col gap-2 p-3">
				<div className="flex flex-col gap-2">
					<h4 className="text-fg-primary font-medium line-height-normal">{item.title}</h4>
					<p className="text-xs text-fg-secondary">
						By
						{' '}
						<span className="text-fg-primary">{item.author}</span>
						{' '}
						on
						{' '}
						{provider}
					</p>
				</div>

				<p className="max-h-22 flex-1 overflow-hidden text-sm text-fg-secondary line-height-snug">{item.description}</p>

				<div className="flex flex-row gap-4 text-xs">
					<Show when={provider !== 'SkyClient'}>
						<div className="flex flex-row items-center gap-2">
							<Download01Icon className="h-4 w-4" />
							{abbreviateNumber(item.downloads)}
						</div>
					</Show>
				</div>
			</div>
		</Link>
	);
}
