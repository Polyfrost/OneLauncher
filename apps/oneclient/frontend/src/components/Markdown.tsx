import { ExternalLink } from '@/components';
import ReactMarkdown from 'react-markdown';
import rehypeRaw from 'rehype-raw';
import remarkGfm from 'remark-gfm';
import { twMerge } from 'tailwind-merge';
import styles from './Markdown.module.css';

export function Markdown({
	body,
	className,
}: {
	body: string;
	className?: string | undefined;
}) {
	return (
		<div className={twMerge(styles.markdown, className)}>
			<ReactMarkdown
				components={{
					a: ({ node, children, ...props }) => <ExternalLink children={children} href={props.href as string} />,
				}}
				rehypePlugins={[rehypeRaw]}
				remarkPlugins={[remarkGfm]}
			>
				{body}
			</ReactMarkdown>
		</div>
	);
}
