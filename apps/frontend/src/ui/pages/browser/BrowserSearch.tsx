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
			<Spinner.Suspense>
				<Show when={results() !== undefined}>
					<SearchResultsContainer
						category="test"
						provider="Modrinth"
						results={results()!.results}
						header={(
							<div class="w-full flex flex-row items-end justify-between">
								<h5 class="ml-2">Results</h5>
								<pagination.Navigation />
							</div>
						)}
					/>
				</Show>

				<div class="mt-2">
					<pagination.Navigation />
				</div>
			</Spinner.Suspense>
		</BrowserContent>
	);
}

export default BrowserSearch;
