import type { Provider, SearchResult } from '@/bindings.gen';
import { abbreviateNumber } from '@/utils';
import { Show } from '@onelauncher/common/components';
import { Download01Icon } from '@untitled-theme/icons-react';

export function PackageGrid({ items, provider }: { items: Array<SearchResult>; provider: Provider }) {
	return (
		<div className="grid grid-cols-2 xl:grid-cols-3 gap-2">
			{items.map(item => (
				<PackageItem key={item.project_id} {...item} provider={provider} />
			))}
		</div>
	);
}

export function PackageItem({ provider, ...item }: SearchResult & { provider: Provider }) {
	function redirect() {
	}

	return (
		<div
			className="h-full min-w-50 flex overflow-hidden rounded-lg bg-component-bg hover:bg-component-bg-hover flex-col max-h-74 min-h-74"
			onClick={redirect}
			tabIndex={0}
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
					<img alt={`Icon for ${item.title}`} className="absolute z-0 max-w-none w-7/6 opacity-50 filter-blur-xl" src={item.icon_url} />
					<img
						alt={`Icon for ${item.title}`}
						className="relative z-1 aspect-ratio-square rounded-md image-render-auto w-2/5"
						src={item.icon_url}
					/>
				</Show>
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
						<span className="text-fg-primary">{provider}</span>
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
		</div>
	);
}
