const PAGE_SEARCH_SELECTOR = 'input[data-page-search-input="true"]:not([disabled]), textarea[data-page-search-input="true"]:not([disabled])';

export function isPageFindShortcut(event: KeyboardEvent): boolean {
  if (event.defaultPrevented || event.altKey) {
    return false;
  }

  const key = event.key.toLowerCase();
  return key === 'f' && (event.ctrlKey || event.metaKey);
}

export function focusPageSearchInput(root: ParentNode = document): boolean {
  const searchInput = root.querySelector<HTMLInputElement | HTMLTextAreaElement>(PAGE_SEARCH_SELECTOR);
  if (!searchInput || searchInput.readOnly) {
    return false;
  }

  searchInput.focus();
  if (typeof searchInput.select === 'function') {
    searchInput.select();
  }

  return true;
}
