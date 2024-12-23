import { type DialogFilter, open } from '@tauri-apps/plugin-dialog';
import { FilePlus02Icon } from '@untitled-theme/icons-solid';
import { createSignal, For, Match, onMount, splitProps, Switch } from 'solid-js';
import Button from './Button';

export interface FilePickerOptions {
	directory?: boolean;
	multiple?: boolean;
	defaultPath?: string;
	title?: string;
	filters?: DialogFilter[];
};

export interface FilePickerRef {
	open: () => void;
}

export default function FilePicker(componentProps: FilePickerOptions & {
	onPicked?: (paths: string[]) => void;
	ref?: (ref: FilePickerRef) => void;
}) {
	const [props, options] = splitProps(componentProps, ['onPicked', 'ref']);
	const [file, setFile] = createSignal<string[]>([]);

	onMount(() => {
		if (options.defaultPath)
			setFile([options.defaultPath]);

		props.ref?.({
			open: openPicker,
		});
	});

	async function openPicker() {
		const path = await open(options) as string[] | string | null; // The type automatically infers from the selected options, but we don't want that here

		if (Array.isArray(path))
			setFile(path);
		else if (path === null)
			setFile([]);
		else
			setFile([path]);

		props.onPicked?.(file());
	}

	return (
		<div class="max-w-full min-w-0 w-full flex flex-row items-center justify-start gap-2">
			<Button
				buttonStyle="secondary"
				class="max-w-full w-full"
				iconLeft={<FilePlus02Icon />}
				onClick={openPicker}
			>
				<div class="box-border max-w-full min-w-0 w-full flex flex-col overflow-x-hidden text-left">
					<Switch>
						<Match when={file().length >= 2}>
							<For each={file()}>
								{path => (
									<span class="overflow-x-hidden text-ellipsis">{path.replaceAll('\\', '/').split('/').pop()}</span>
								)}
							</For>
						</Match>
						<Match when={file().length === 1}>
							<span class="overflow-x-hidden text-ellipsis">{file()[0]}</span>
						</Match>
						<Match when>
							<span class="overflow-x-hidden text-ellipsis">
								No
								{' '}
								{options.directory ? 'Folder' : 'File'}
								{options.multiple ? 's' : ''}
								{' '}
								Selected
							</span>
						</Match>
					</Switch>
				</div>
			</Button>
		</div>
	);
}
