import type { ParentProps } from 'solid-js';
import { renderHighlightedString } from '@onelauncher/client';
import styles from './Markdown.module.scss';

export type MarkdownProps = ParentProps & {
	body: string;
};

// eslint-disable-next-line solid/no-innerhtml -- we use xss to ensure there is not any issues
const Markdown = (props: MarkdownProps) => <div class={styles.markdown} innerHTML={renderHighlightedString(props.body)} />;

export default Markdown;
