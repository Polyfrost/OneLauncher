import { useSearchParams } from '@solidjs/router';
import ScrollableContainer from '~ui/components/ScrollableContainer';

function BrowserCategory() {
	const [params] = useSearchParams<{
		category: string;
	}>();

	return (
		<div>
			<h1>
				Category =
				{' '}
				{params.category}
			</h1>
			<ScrollableContainer>
				<span>test</span>
			</ScrollableContainer>
		</div>
	);
}

export default BrowserCategory;
