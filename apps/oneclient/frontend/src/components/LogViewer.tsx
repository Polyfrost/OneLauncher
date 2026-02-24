import { useVirtualizer } from '@tanstack/react-virtual';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import { useCallback, useEffect, useImperativeHandle, useRef, useState } from 'react';
import styles from './LogViewer.module.css';

export interface LogViewerProps {
	content: string;
	scrollRef: React.RefObject<HTMLElement | null>;
	autoScroll?: boolean;
	maxLines?: number;
	onTrimmedLines?: (trimmedLines: number) => void;
	ref?: React.Ref<LogViewerRef>;
}

export interface LogViewerRef {
	push: (line: string) => void;
	scrollToEnd: () => void;
}

const LINE_HEIGHT = 16;
const REGEX_PATTERN = /\[((?:\S+ )?\d+:\d+:\d+(?:\.\d+)?)\] \[(.[^(\n\r/\u2028\u2029]*)\/(\w+)\]:? (?:\[(CHAT)\])?/;

function normalizeLineLimit(maxLines?: number): number {
	if (maxLines === undefined || !Number.isFinite(maxLines))
		return Number.POSITIVE_INFINITY;

	return Math.max(1, Math.floor(maxLines));
}

interface TrimResult {
	lines: Array<string>;
	trimmedCount: number;
}

function trimLines(
	lines: Array<string>,
	lineLimit: number,
): TrimResult {
	if (!Number.isFinite(lineLimit) || lines.length <= lineLimit)
		return {
			lines,
			trimmedCount: 0,
		};

	const trimmedCount = lines.length - lineLimit;
	return {
		lines: lines.slice(trimmedCount),
		trimmedCount,
	};
}

export function LogViewer({
	content,
	scrollRef,
	autoScroll = false,
	maxLines,
	onTrimmedLines,
	ref,
}: LogViewerProps) {
	const lineLimit = normalizeLineLimit(maxLines);
	const containerRef = useRef<HTMLDivElement>(null);
	const pendingLinesRef = useRef<Array<string>>([]);
	const frameRef = useRef<number | null>(null);
	const [lines, setLines] = useState<Array<string>>(() => trimLines(content.split('\n'), lineLimit).lines);
	const linesRef = useRef<Array<string>>(lines);

	const virtualizer = useVirtualizer({
		count: lines.length,
		estimateSize: () => LINE_HEIGHT,
		getScrollElement: () => scrollRef.current ?? null,
		paddingStart: 4,
		paddingEnd: 4,
		gap: 0,
		overscan: 8,
		scrollMargin: containerRef.current ? containerRef.current.offsetTop + 75 : 0,
	});

	const scrollToEnd = useCallback(() => {
		requestAnimationFrame(() => scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight }));
	}, [scrollRef]);

	useEffect(() => {
		pendingLinesRef.current = [];
		if (frameRef.current !== null) {
			cancelAnimationFrame(frameRef.current);
			frameRef.current = null;
		}

		const nextState = trimLines(content.split('\n'), lineLimit);
		linesRef.current = nextState.lines;
		setLines(nextState.lines);

		if (nextState.trimmedCount > 0)
			onTrimmedLines?.(nextState.trimmedCount);
	}, [content, lineLimit, onTrimmedLines]);

	useEffect(() => {
		return () => {
			if (frameRef.current !== null)
				cancelAnimationFrame(frameRef.current);
		};
	}, []);

	const appendLine = useCallback((line: string) => {
		pendingLinesRef.current.push(line);
		if (frameRef.current !== null)
			return;

		frameRef.current = requestAnimationFrame(() => {
			frameRef.current = null;
			if (pendingLinesRef.current.length === 0)
				return;

			const pending = pendingLinesRef.current;
			pendingLinesRef.current = [];

			const nextState = trimLines(linesRef.current.concat(pending), lineLimit);
			linesRef.current = nextState.lines;
			setLines(nextState.lines);

			if (nextState.trimmedCount > 0)
				onTrimmedLines?.(nextState.trimmedCount);

			if (autoScroll)
				scrollToEnd();
		});
	}, [autoScroll, lineLimit, onTrimmedLines, scrollToEnd]);

	useImperativeHandle(ref, () => ({
		push: appendLine,
		scrollToEnd,
	}), [appendLine, scrollToEnd]);

	return (
		<div
			ref={containerRef}
			style={{
				height: `${virtualizer.getTotalSize()}px`,
			}}
		>
			<OverlayScrollbarsComponent
				className={styles.log_container}
				options={{
					overflow: {
						x: 'scroll',
						y: 'hidden',
					},
				}}
				style={{
					height: '100%',
				}}
			>
				<code className={styles.log}>
					{virtualizer.getVirtualItems().map(virtualRow => (
						<LogLine
							key={virtualRow.key}
							line={lines[virtualRow.index]}
							style={{
								height: `${virtualRow.size}px`,
								transform: `translateY(${virtualRow.start - virtualizer.options.scrollMargin}px)`,
							}}
						/>
					))}
				</code>
			</OverlayScrollbarsComponent>
		</div>
	);
}

function LogLine({
	line,
	style,
}: {
	line: string;
	style: React.CSSProperties;
}) {
	line = line.replace(/§./g, ''); // remove color codes
	const groups = line.match(REGEX_PATTERN);

	if (groups === null)
		return (
			<span style={style}>
				{line}
				<br />
			</span>
		);

	const [matched, time, threadName, level, isChat] = groups as [string, string, string, string, string | undefined];

	return (
		<span data-level={level} style={style}>
			<span data-time={time}>[{time}]</span>
			<span>[{threadName}/{level}]</span>
			<span data-chat={isChat ? 'true' : undefined}>{isChat ? ' [CHAT]' : ' '}{line.slice(matched.length)}</span>
			<br />
		</span>
	);
}
