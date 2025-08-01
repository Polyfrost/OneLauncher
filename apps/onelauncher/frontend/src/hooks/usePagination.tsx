import { Button, Show } from '@onelauncher/common/components';
import { ChevronLeftIcon, ChevronRightIcon } from '@untitled-theme/icons-react';
import { useState } from 'react';

export interface PaginationOptions {
	itemsCount: number;
	itemsPerPage: number;
	initialPage?: number;
};

function usePagination(options: PaginationOptions) {
	// const [options, setOptions] = useState(initialOptions);

	const [page, setPage] = useState(Math.max(1, options.initialPage || 1));

	const next = () => setPage(page => page + 1);
	const prev = () => setPage(page => page - 1);

	const totalPages = Math.ceil(options.itemsCount / options.itemsPerPage);

	const hasNext = page < totalPages;
	const hasPrev = page > 1;

	const offset = (page - 1) * options.itemsPerPage;

	// Exported
	function reset(/* options: PaginationOptions */) {
		setPage(1);
	}

	function getVisiblePageNumbers(page: number) {
		const pages: Array<number> = [];

		const left = page - 1 > 1 ? page - 1 : null;
		const right = page + 1 < totalPages ? page + 1 : null;

		if (left !== null)
			pages.push(left);

		if (page !== 1 && page !== totalPages)
			pages.push(page);

		if (right !== null)
			pages.push(right);

		return pages;
	}

	function PaginationBtn({ page: pageIndex }: { page: number }) {
		return (
			<Button
				className="p-2 min-w-8 h-8"
				color={pageIndex === page ? 'primary' : 'secondary'}

				onClick={() => {
					if (pageIndex === page)
						return;

					setPage(pageIndex);
				}}
			>
				<span className="text-trim">{pageIndex}</span>
			</Button>
		);
	}

	const Navigation = () => (
		<div className="flex flex-row items-center justify-center gap-x-1">

			<Button
				children={<ChevronLeftIcon />}
				color="secondary"
				isDisabled={page <= 1}
				onClick={prev}
				size="icon"
			/>

			<PaginationBtn page={1} />

			<Show when={page > 1 + 2}>
				<span>...</span>
			</Show>

			{getVisiblePageNumbers(page).map(page => <PaginationBtn key={page} page={page} />)}

			<Show when={page < totalPages - 2}>
				<span>...</span>
			</Show>

			<Show when={totalPages > 1}>
				<PaginationBtn page={totalPages} />
			</Show>

			<Button
				children={<ChevronRightIcon />}
				color="secondary"
				isDisabled={page >= totalPages}
				onClick={next}
				size="icon"
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
		options,
		Navigation,
	};
}

export default usePagination;
