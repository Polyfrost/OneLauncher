import OneConfigLogo from '@/assets/logos/oneconfig.svg';
import { Button, Show } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { ChevronRightIcon } from '@untitled-theme/icons-react';
import { BrowserLayout } from './route';

export const Route = createFileRoute('/app/browser/')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<BrowserLayout>
			<div className="flex flex-col gap-8">
				<Featured />
			</div>
		</BrowserLayout>
	);
}

function Featured() {
	return (
		<Show when>
			<div className="flex flex-col gap-y-1">
				<h5 className="ml-2">Featured</h5>
				<div className="w-full flex flex-row overflow-hidden rounded-lg bg-page-elevated">
					<div className="w-full p-1">
						<img alt="thumbnail" className="aspect-ratio-video h-full rounded-md object-cover object-center" src="https://cdn.modrinth.com/data/AANobbMI/295862f4724dc3f78df3447ad6072b2dcd3ef0c9_96.webp" />
					</div>
					<div className="max-w-84 min-w-52 flex flex-col gap-y-1 p-4">
						<h2>Sodium</h2>

						<Show when={false}>
							<div className="w-fit flex flex-row items-center gap-x-1 rounded-lg bg-border/10 px-1.5 py-1 text-fg-primary transition hover:opacity-80">
								<img alt="OneConfig Logo" className="h-3.5 w-3.5" src={OneConfigLogo} />
								<span className="text-sm font-medium">OneConfig Integrated</span>
							</div>
						</Show>

						<p className="mt-1 flex-1 leading-normal">Peak mod</p>

						<div className="flex flex-row justify-end">
							<Button color="ghost">
								View Package
								<ChevronRightIcon />
							</Button>
						</div>
					</div>
				</div>
			</div>
		</Show>
	);
}
