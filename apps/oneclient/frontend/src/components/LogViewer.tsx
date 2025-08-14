import { useVirtualizer } from '@tanstack/react-virtual';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import { useImperativeHandle, useRef, useState } from 'react';
import styles from './LogViewer.module.css';

export interface LogViewerProps {
	content: string;
	scrollRef: React.RefObject<HTMLElement | null>;
	ref?: React.RefObject<LogViewerRef>;
}

export interface LogViewerRef {
	push: (line: string) => void;
}

const LINE_HEIGHT = 16;
const REGEX_PATTERN = /\[((?:\S+ )?\d+:\d+:\d+(?:\.\d+)?)\] \[(.[^(\n\r/\u2028\u2029]*)\/(\w+)\]:? (?:\[(CHAT)\])?/;

export function LogViewer({
	content,
	scrollRef,
	ref,
}: LogViewerProps) {
	const linesRef = useRef<Array<string>>(content.split('\n'));

	// used to force re-render (and re-init of virtualizer hook)
	const [lastUpdated, setLastUpdated] = useState(Date.now());

	const virtualizer = useVirtualizer({
		count: linesRef.current.length,
		estimateSize: () => LINE_HEIGHT,
		getScrollElement: () => scrollRef.current ?? null,
		paddingStart: 4,
		paddingEnd: 4,
		gap: 0,
		overscan: 20,
	});

	useImperativeHandle(ref, () => ({
		push: (line: string) => {
			linesRef.current.push(line);
			setLastUpdated(Date.now());
		},
	}), []);

	return (
		<div
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
							line={linesRef.current[virtualRow.index]}
							style={{
								height: `${virtualRow.size}px`,
								transform: `translateY(${virtualRow.start}px)`,
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
			<span data-chat={isChat}>{isChat ? ' [CHAT] ' : ' '}{line.slice(matched.length)}</span>
			<br />
		</span>
	);
}
