import type { Provider, SearchResult } from '@/bindings.gen';
import { Link } from '@tanstack/react-router';

export function PackageGrid({ items, provider, clusterId }: { items: Array<SearchResult>; provider: Provider; clusterId: number }) {
	return (
		<div className="grid grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-5! gap-4">
			{items.map(item => (
				<PackageItem
					key={item.project_id}
					{...item}
					clusterId={clusterId}
					provider={provider}
				/>
			))}
		</div>
	);
}

export function PackageItem({ provider, clusterId, ...item }: SearchResult & { provider: Provider; clusterId: number }) {
	return (
		<Link
			className="h-full min-w-50 overflow-hidden rounded-lg bg-component-bg hover:bg-component-bg-hover grid grid-rows-[7rem_auto] max-h-74 min-h-74"
			preloadDelay={750}
			search={{ provider, packageId: item.project_id, clusterId }}
			tabIndex={0}
			to="/app/cluster/browser/package"
		>
			<div
				className="relative flex items-center justify-center overflow-hidden w-full h-28"
			>
				{item.icon_url
					? (
							<>
								<img alt={`Icon for ${item.title}`} className="absolute z-0 max-w-none w-7/6 opacity-50 blur-xl" src={item.icon_url} />
								<img
									alt={`Icon for ${item.title}`}
									className="relative z-1 aspect-ratio-square rounded-lg image-render-auto h-5/6"
									src={item.icon_url}
								/>
							</>
						)
					: (
							<div
								className="aspect-ratio-square rounded-md bg-border/05 w-2/5"
							/>
						)}
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
			</div>
		</Link>
	);
}
