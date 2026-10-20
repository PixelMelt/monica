// `PaginatorHelper::getData` from a list response's `meta`, with the links
// of Laravel's `LengthAwarePaginator::linkCollection` (UrlWindow, 3 pages
// on each side). `to` is the current route: page links keep its query.

import { trans } from 'laravel-vue-i18n';

const ON_EACH_SIDE = 3;

const range = (from, to) => Array.from({ length: to - from + 1 }, (_, i) => from + i);

function urlWindow(current, last) {
  if (last < ON_EACH_SIDE * 2 + 8) return { first: range(1, last), slider: null, last: null };
  const window = ON_EACH_SIDE + 4;
  if (current <= window) {
    return { first: range(1, window + ON_EACH_SIDE), slider: null, last: range(last - 1, last) };
  }
  if (current > last - window) {
    return { first: range(1, 2), slider: null, last: range(last - (window + (ON_EACH_SIDE - 1)), last) };
  }
  return {
    first: range(1, 2),
    slider: range(current - ON_EACH_SIDE, current + ON_EACH_SIDE),
    last: range(last - 1, last),
  };
}

export function paginator(meta, count, to) {
  const url = (page) => {
    const query = new URLSearchParams(to.query);
    query.set('page', page);
    return `${to.path}?${query}`;
  };
  const current = meta.page;
  const lastPage = Math.max(meta.last_page, 1);
  const firstItem = count > 0 ? (current - 1) * meta.per_page + 1 : null;
  const w = urlWindow(current, lastPage);
  const elements = [w.first, w.slider ? '...' : null, w.slider, w.last ? '...' : null, w.last].filter(Boolean);
  const previousPageUrl = current > 1 ? url(current - 1) : null;
  const nextPageUrl = current < lastPage ? url(current + 1) : null;
  const links = [{ url: previousPageUrl, label: trans('pagination.previous'), active: false }];
  for (const element of elements) {
    if (element === '...') {
      links.push({ url: null, label: '...', active: false });
    } else {
      for (const page of element) links.push({ url: url(page), label: String(page), active: page === current });
    }
  }
  links.push({ url: nextPageUrl, label: trans('pagination.next'), active: false });
  return {
    count,
    currentPage: current,
    firstItem,
    firstPageUrl: url(1),
    hasMorePages: current < lastPage,
    lastItem: firstItem === null ? null : firstItem + count - 1,
    lastPage,
    lastPageUrl: url(lastPage),
    links,
    nextPageUrl,
    onFirstPage: current <= 1,
    path: to.path,
    perPage: meta.per_page,
    previousPageUrl,
    total: meta.total,
  };
}
