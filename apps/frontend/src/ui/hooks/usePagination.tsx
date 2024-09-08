import { ChevronLeftIcon, ChevronRightIcon } from '@untitled-theme/icons-solid';
import Button from '~ui/components/base/Button';
import { createSignal, For, Show } from 'solid-js';

export interface PaginationOptions {
	itemsCount: () => number;
	itemsPerPage: () => number;
	initialPage?: () => number;
};

function usePagination(initialOptions: PaginationOptions) {
	const [options, setOptions] = createSignal(initialOptions);

	// eslint-disable-next-line solid/reactivity -- -
	const [page, setPage] = createSignal(Math.max(1, options().initialPage?.() || 1));

	const next = () => setPage(page => page + 1);
	const prev = () => setPage(page => page - 1);

	const totalPages = () => Math.ceil(options().itemsCount() / options().itemsPerPage());

	const hasNext = () => page() < totalPages();
	const hasPrev = () => page() > 1;

	const offset = () => (page() - 1) * options().itemsPerPage();

	// Exported
	function reset(options: PaginationOptions) {
		setOptions(options);
		setPage(1);
	}

	function getVisiblePageNumbers(page: number) {
		const pages: number[] = [];

		const left = page - 1 > 1 ? page - 1 : null;
		const right = page + 1 < totalPages() ? page + 1 : null;

		if (left !== null)
			pages.push(left);

		if (page !== 1 && page !== totalPages())
			pages.push(page);

		if (right !== null)
			pages.push(right);

		return pages;
	}

	function PaginationBtn(props: { page: number }) {
		return (
			<Button
				buttonStyle={props.page === page() ? 'iconPrimary' : 'iconSecondary'}
				children={props.page}
				onClick={() => {
					if (props.page === page())
						return;

					setPage(props.page);
				}}
			/>
		);
	}

	const Navigation = () => (
		<div class="flex flex-row items-center justify-center gap-x-1">
			<Button
				buttonStyle="iconSecondary"
				children={<ChevronLeftIcon />}
				disabled={page() <= 1}
				onClick={() => setPage(page => page - 1)}
			/>

			<PaginationBtn page={1} />

			<Show when={page() > 1 + 2}>
				<span>...</span>
			</Show>

			<For each={getVisiblePageNumbers(page())}>
				{page => <PaginationBtn page={page} />}
			</For>

			<Show when={page() < totalPages() - 2}>
				<span>...</span>
			</Show>

			<Show when={totalPages() > 1}>
				<PaginationBtn page={totalPages()} />
			</Show>

			<Button
				buttonStyle="iconSecondary"
				children={<ChevronRightIcon />}
				disabled={page() >= totalPages()}
				onClick={() => setPage(page => page + 1)}
			/>
		</div>
	);

	return {
		page,
		next,
		prev,
		hasNext,
		hasPrev,
		totalPages,
		offset,
		reset,
		Navigation,
	};
}

export default usePagination;
