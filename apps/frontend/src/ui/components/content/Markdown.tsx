import { type ParentProps, mergeProps, splitProps } from 'solid-js';
import { SolidMarkdown, type SolidMarkdownOptions } from 'solid-markdown';
import Link from '../base/Link';
import styles from './Markdown.module.scss';

export type MarkdownProps = ParentProps & Partial<SolidMarkdownOptions>;

function Markdown(props: MarkdownProps) {
	const [split, rest] = splitProps(props, ['class']);

	const defaultOpts: MarkdownProps = {
		renderingStrategy: 'memo',
		components: {
			a: Link,
		},
	};

	const merged = mergeProps(defaultOpts, rest);

	return (
		<SolidMarkdown
			class={`${styles.markdown} ${split.class || ''}`}
			{...merged}
		/>
	);
}

export default Markdown;
