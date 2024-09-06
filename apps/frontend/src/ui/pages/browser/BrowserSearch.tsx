import { Show, createEffect, on } from 'solid-js';
import { BrowserContent } from './BrowserRoot';
import { bridge } from '~imports';
import SearchResultsContainer from '~ui/components/content/SearchResults';
import Spinner from '~ui/components/Spinner';
import useBrowser from '~ui/hooks/useBrowser';
import useCommand from '~ui/hooks/useCommand';
import usePagination, { type PaginationOptions } from '~ui/hooks/usePagination';

function BrowserSearch() {
	const browser = useBrowser();
	const [results, { refetch }] = useCommand(() => bridge.commands.searchProviderPackages('Modrinth', browser.searchQuery()));

	const paginationOptions = (): PaginationOptions => ({
		itemsCount: () => results()?.total || 0,
		itemsPerPage: () => browser.searchQuery().limit || 20,
	});

	const pagination = usePagination(paginationOptions());

	createEffect(on(browser.searchQuery, () => {
		refetch();
	}));

	createEffect(on(() => results()?.total, (curr, prev) => {
		if (curr !== prev)
			pagination.reset(paginationOptions());
	}));

	createEffect(on(pagination.page, () => {
		browser.setSearchQuery(prev => ({
			...prev,
			offset: pagination.offset(),
		}));
	}));

	return (
		<BrowserContent>
			<pagination.Navigation />

			<div class="my-2">
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
			</div>

			<pagination.Navigation />
		</BrowserContent>
	);
}

export default BrowserSearch;
