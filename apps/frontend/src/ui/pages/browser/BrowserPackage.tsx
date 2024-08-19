import { Show } from 'solid-js';
import { type Params, useSearchParams } from '@solidjs/router';
import { Download01Icon, HeartIcon, LinkExternal01Icon } from '@untitled-theme/icons-solid';
import type { ManagedPackage, Providers } from '~bindings';
import { bridge } from '~imports';
import useCommand from '~ui/hooks/useCommand';
import { abbreviateNumber, formatAsRelative, openLicense } from '~utils';
import Tooltip from '~ui/components/base/Tooltip';
import Markdown from '~ui/components/content/Markdown';

interface BrowserModParams extends Params {
	id: string;
	provider: Providers;
}

function BrowserPackage() {
	const [params] = useSearchParams<BrowserModParams>();
	const [pkg] = useCommand(bridge.commands.getPackage, params.provider!, params.id!);

	// function testDownload() {
	// 	const packag = pkg();
	// 	if (!packag)
	// 		return;

	// 	bridge.commands.downloadPackage(
	// 		packag.provider,
	// 		packag.id,
	// 		'33f4cd3a-ff62-48bf-a777-fdbf35699cf5',
	// 		null,
	// 		null,
	// 		null,
	// 	);
	// }

	return (
		<>
			<div class="flex flex-row gap-x-4">
				{/* TODO: Make a progress bar of some sort */}
				<Show
					when={pkg() !== undefined}
					fallback={<div>Loading...</div>}
					children={(
						<>
							<BrowserSidebar {...pkg()!} />

							<div class="flex flex-1 flex-col items-start justify-between gap-y-4 rounded-lg bg-component-bg p-4 px-6">
								{/* <div class="w-full flex flex-row items-center justify-between">
									<h1>{pkg()!.title}</h1>
									<Button
										buttonStyle="primary"
										iconLeft={<Download01Icon />}
										onClick={testDownload}
										children="Install to..."
									/>
								</div> */}

								<div class="flex-1">
									<Markdown>
										{pkg()!.body}
									</Markdown>
								</div>

							</div>
						</>
					)}
				/>
			</div>
		</>
	);
}

BrowserPackage.buildUrl = function (params: BrowserModParams): string {
	return `/browser/package?id=${params.id}&provider=${params.provider}`;
};

export default BrowserPackage;

function BrowserSidebar(props: ManagedPackage) {
	const createdAt = () => new Date(props.created);
	const updatedAt = () => new Date(props.updated);

	return (
		<div class="max-w-60 min-w-54 flex flex-col gap-y-4">
			<div class="min-h-72 flex flex-col overflow-hidden rounded-lg bg-component-bg">
				<div class="relative h-28 flex items-center justify-center overflow-hidden">
					<img class="absolute z-0 max-w-none w-7/6 filter-blur-xl" src={props.icon_url || ''} alt={`Icon for ${props.title}`} />
					<img class="relative z-1 aspect-ratio-square w-2/5 rounded-md image-render-auto" src={props.icon_url || ''} alt={`Icon for ${props.title}`} />
				</div>
				<div class="flex flex-1 flex-col gap-2 p-3">
					<div class="flex flex-col gap-2">
						<h4 class="text-fg-primary font-medium">{props.title}</h4>
						<p class="text-xs text-fg-secondary">
							<span class="text-fg-primary capitalize">{props.package_type}</span>
							{' '}
							on
							{' '}
							<span class="text-fg-primary">{props.provider}</span>
						</p>
					</div>

					<p class="max-h-22 flex-1 overflow-hidden text-sm text-fg-secondary line-height-snug">{props.description}</p>

					<div class="flex flex-row gap-4 text-xs">
						<div class="flex flex-row items-center gap-2">
							<Download01Icon class="h-4 w-4" />
							{abbreviateNumber(props.downloads)}
						</div>

						<div class="flex flex-row items-center gap-2">
							<HeartIcon class="h-4 w-4" />
							{abbreviateNumber(props.followers)}
						</div>
					</div>
				</div>
			</div>

			<div class="flex flex-col gap-2 rounded-lg bg-component-bg p-3">
				<h4 class="text-fg-primary font-medium">Links</h4>
				{/* TODO: Links */}
			</div>

			<div class="flex flex-col gap-2 rounded-lg bg-component-bg p-3">
				<h4 class="text-fg-primary font-medium">Authors</h4>

			</div>

			<div class="flex flex-col gap-2 rounded-lg bg-component-bg p-3">
				<h4 class="text-fg-primary font-medium">Details</h4>
				<Show when={props.license !== null}>
					<div class="flex flex-row items-start gap-x-1">
						License
						<div
							class="text-link hover:text-link-hover flex flex-1 flex-row items-start gap-x-1 font-bold"
							onClick={() => openLicense(props.license)}
						>
							{props.license?.name || props.license?.id || 'Unknown'}
							<LinkExternal01Icon class="h-3.5! w-3.5!" />
						</div>
					</div>
				</Show>

				<Tooltip text={createdAt().toLocaleString()}>
					Created
					{' '}
					<span class="text-fg-primary font-medium">
						{formatAsRelative(createdAt().getTime(), 'en', 'long')}
					</span>
				</Tooltip>

				<Tooltip text={updatedAt().toLocaleString()}>
					Last Updated
					{' '}
					<span class="text-fg-primary font-medium">
						{formatAsRelative(updatedAt().getTime(), 'en', 'long')}
					</span>
				</Tooltip>
			</div>

		</div>
	);
}
