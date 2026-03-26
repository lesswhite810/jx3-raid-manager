import { describe, expect, test } from 'vitest';
import { focusPageSearchInput, isPageFindShortcut } from './pageSearchUtils';

function createKeyboardEvent(overrides: Partial<KeyboardEvent> = {}): KeyboardEvent {
  return {
    key: '',
    ctrlKey: false,
    metaKey: false,
    altKey: false,
    defaultPrevented: false,
    ...overrides,
  } as KeyboardEvent;
}

interface MockSearchInput {
  focusCalled: boolean;
  selectCalled: boolean;
  readOnly: boolean;
  focus: () => void;
  select: () => void;
}

function createMockSearchInput(readOnly = false): HTMLInputElement & MockSearchInput {
  return {
    focusCalled: false,
    selectCalled: false,
    readOnly,
    focus() {
      this.focusCalled = true;
    },
    select() {
      this.selectCalled = true;
    },
  } as HTMLInputElement & MockSearchInput;
}

function createMockRoot(searchInput: HTMLInputElement | HTMLTextAreaElement | null): ParentNode {
  return {
    querySelector: () => searchInput,
  } as ParentNode;
}

describe('isPageFindShortcut', () => {
  test('识别 Ctrl+F 与 Cmd+F', () => {
    expect(isPageFindShortcut(createKeyboardEvent({ key: 'f', ctrlKey: true }))).toBe(true);
    expect(isPageFindShortcut(createKeyboardEvent({ key: 'F', metaKey: true }))).toBe(true);
  });

  test('忽略带额外修饰键或其他按键的情况', () => {
    expect(isPageFindShortcut(createKeyboardEvent({ key: 'f', ctrlKey: true, altKey: true }))).toBe(false);
    expect(isPageFindShortcut(createKeyboardEvent({ key: 'g', ctrlKey: true }))).toBe(false);
  });
});

describe('focusPageSearchInput', () => {
  test('聚焦并选中当前页面的搜索输入框', () => {
    const searchInput = createMockSearchInput();

    const focused = focusPageSearchInput(createMockRoot(searchInput));

    expect(focused).toBe(true);
    expect(searchInput.focusCalled).toBe(true);
    expect(searchInput.selectCalled).toBe(true);
  });

  test('忽略只读的搜索输入框', () => {
    const searchInput = createMockSearchInput(true);

    const focused = focusPageSearchInput(createMockRoot(searchInput));

    expect(focused).toBe(false);
    expect(searchInput.focusCalled).toBe(false);
    expect(searchInput.selectCalled).toBe(false);
  });

  test('没有可用搜索输入框时返回 false', () => {
    expect(focusPageSearchInput(createMockRoot(null))).toBe(false);
  });
});
