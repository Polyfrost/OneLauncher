import { ChevronDownDoubleIcon } from '@untitled-theme/icons-solid';
import { OverlayScrollbarsComponent, type OverlayScrollbarsComponentRef } from 'overlayscrollbars-solid';
import { createEffect, createMemo, createSignal, Index, Show, untrack } from 'solid-js';
import type { Ref } from '@solid-primitives/refs';
import type { OnUpdatedEventListenerArgs, OverlayScrollbars } from 'overlayscrollbars';
import Button from '../base/Button';
import styles from './FormattedLog.module.scss';

interface FormattedLogProps {
	log?: string | undefined;
	codeRef?: Ref<HTMLElement>;
	enableAutoScroll?: boolean;
};

function FormattedLog(props: FormattedLogProps) {
	// TODO(perf): Do a "infinite scroll" method of rendering the log. Adding 15918590815 dom elements at once lags the render thread for a bit
	const lines = createMemo(() => (props.log?.trim().split('\n') || []));
	const [shouldScroll, setShouldScroll] = createSignal(false);

	createEffect(() => {
		setShouldScroll(props.enableAutoScroll === true);
	});

	let overlayScrollbars!: OverlayScrollbarsComponentRef;

	function autoScrollUpdated(instance: OverlayScrollbars, _args: OnUpdatedEventListenerArgs) {
		if (untrack(shouldScroll) === true)
			instance.elements().scrollOffsetElement.scrollTop = instance.elements().scrollOffsetElement.scrollHeight;
	}

	function autoScrollInitialized(instance: OverlayScrollbars) {
		instance.elements().scrollEventElement.addEventListener('wheel', () => {
			toggleAutoScroll(false);
		});
	}

	function toggleAutoScroll(value: boolean) {
		setShouldScroll(value);
		if (value === true)
			overlayScrollbars.osInstance()?.update(true);
	}

	return (
		<div class="relative h-full flex flex-1 flex-col overflow-hidden rounded-md bg-component-bg">
			<Show when={props.enableAutoScroll === true}>
				<div class="absolute right-0 top-0 z-1 h-6 w-full flex flex-row justify-end border border-gray-05 rounded-t-md bg-page p-px">
					<Button.Toggle
						buttonStyle="icon"
						checked={shouldScroll}
						children={<ChevronDownDoubleIcon />}
						class="h-full! rounded-md! py-1!"
						onChecked={toggleAutoScroll}
					/>
				</div>
			</Show>

			<div
				class={`${props.enableAutoScroll === true ? 'mt-6' : ''} h-full flex flex-1 overflow-auto font-medium font-mono`}
			>
				<OverlayScrollbarsComponent
					class="relative h-full flex-1"
					events={{
						updated: autoScrollUpdated,
						initialized: autoScrollInitialized,
					}}
					ref={overlayScrollbars}
				>
					<code class={styles.log} ref={props.codeRef}>
						<Index each={lines()}>
							{(line, index) => {
								if (index === lines().length - 1)
									return <></>;

								return <Line line={line()} />;
							}}
						</Index>
					</code>
				</OverlayScrollbarsComponent>
			</div>
		</div>
	);
}

interface LineProps {
	line: string;
};

/**
 * The first part of the pattern = `\[(\d+:\d+:\d+)\]`, attempts to find
 * and group `[DD:DD:DD]` where DD is a digit.
 * It matches only the formatted time (without the brackets)
 *
 * The second part of the pattern = `\[.[^\n\r/\u2028\u2029]*\/(.+)\]`,
 * attempts to find and group `[Thread name/LEVEL]`
 * where LEVEL is commonly either `INFO`, `WARN`, `ERROR`.
 * It only matches the Thread name and LEVEL
 *
 * The third part of the pattern = `: (?:\[(CHAT)\])?`,
 * attempts to find and group `: [CHAT]`.
 * It only matches `CHAT` if it is present.
 * This is an **optional** capture group.
 */
const REGEX_PATTERN = /\[(\d+:\d+:\d+)\] \[(.[^(\n\r/\u2028\u2029]*)\/(\w+)\]:? (?:\[(CHAT)\])?/;

export function Line(props: LineProps) {
	const format = (line: string) => {
		line = line.replace(/§./g, '');
		const prefix = line.match(REGEX_PATTERN);

		const isEmpty = line.trim() === '';

		if (isEmpty)
			line = '\n';

		if (prefix === null)
			return (
				<span
					{...(isEmpty ? { 'data-empty': 'true' } : {})}
				>
					{line}
				</span>
			);

		const isChatMsg = prefix[4] === 'CHAT';

		return (
			<span
				data-level={prefix[3]?.toUpperCase()}
				{...(isChatMsg ? { 'data-chat': 'true' } : {})}
				{...(isEmpty ? { 'data-empty': 'true' } : {})}
			>
				<span data-date={prefix[1]}>{`[${prefix[1]}]`}</span>
				<span>{` [${prefix[2]}/${prefix[3]}]: ${isChatMsg ? `[${prefix[4]}]` : ''}`}</span>
				<span>{line.slice(prefix[0].length)}</span>
			</span>
		);
	};

	return (
		<>{format(props.line)}</>
	);
}

export default FormattedLog;
