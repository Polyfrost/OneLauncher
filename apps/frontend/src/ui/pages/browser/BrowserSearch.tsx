import { Show, createEffect, on } from 'solid-js';
import { BrowserContent } from './BrowserRoot';
import { bridge } from '~imports';
import SearchResultsContainer from '~ui/components/content/SearchResults';
import Spinner from '~ui/components/Spinner';
import useBrowser from '~ui/hooks/useBrowser';
import useCommand from '~ui/hooks/useCommand';

function BrowserSearch() {
	const browser = useBrowser();

	const [results, { refetch }] = useCommand(() => bridge.commands.searchProviderPackages('Modrinth', browser.searchQuery()));

	createEffect(on(() => browser.searchQuery().query, () => {
		refetch();
	}));

	return (
		<BrowserContent>
			<Spinner.Suspense>
				<Show when={results() !== undefined}>
					<SearchResultsContainer
						category="test"
						header="Results"
						provider="Modrinth"
						results={results()!.results}
					/>
				</Show>
			</Spinner.Suspense>
		</BrowserContent>
	);
}

export default BrowserSearch;
