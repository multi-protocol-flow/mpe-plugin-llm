import React, { useState, useRef, useEffect, useLayoutEffect, useMemo, useCallback } from 'react';
import { IconBraces, IconSearch, IconX, IconGlobe } from './icons';
import { t } from './i18n';

export interface VariablePickerProps {
  /** Map of flow/environment variables: { [varName]: value } */
  variables?: Record<string, unknown>;
  /** Callback when a variable expression is selected (without {{}} or with {{}}) */
  onSelect: (varExpression: string) => void;
  /** Optional target input/textarea ref to automatically insert at cursor */
  inputRef?: React.RefObject<HTMLInputElement | HTMLTextAreaElement | null>;
  /** Optional current text value when using inputRef */
  currentValue?: string;
  /** Optional onChange when using inputRef */
  onValueChange?: (newValue: string) => void;
  /** Button title / tooltip */
  title?: string;
  /** Button variant: 'sm' | 'icon' | 'badge' */
  variant?: 'sm' | 'icon' | 'badge';
  /** Custom button label */
  label?: string;
  /** Custom button class */
  className?: string;
}

/**
 * Inserts `{{expression}}` into the input/textarea at the current cursor position.
 */
export function insertVariableIntoInput(
  el: HTMLInputElement | HTMLTextAreaElement | null,
  currentValue: string,
  varExpression: string,
  onChange?: (val: string) => void
): string {
  const insertion = `{{${varExpression}}}`;
  if (!el) {
    const next = (currentValue || '') + insertion;
    onChange?.(next);
    return next;
  }

  const start = el.selectionStart ?? currentValue.length;
  const end = el.selectionEnd ?? currentValue.length;
  const next = currentValue.slice(0, start) + insertion + currentValue.slice(end);
  onChange?.(next);

  requestAnimationFrame(() => {
    try {
      el.focus();
      const newPos = start + insertion.length;
      el.setSelectionRange(newPos, newPos);
    } catch {
      // Ignore if element is not focusable
    }
  });

  return next;
}

/** Gap between the trigger button and the dropdown (px). */
const DROPDOWN_GAP = 4;
/** Keep the dropdown this far from the viewport edges (px). */
const VIEWPORT_MARGIN = 8;
/** Must match `.var-picker-body { max-height }` — the list scrolls past this. */
const DROPDOWN_MAX_BODY_HEIGHT = 250;
/** Never shrink below this, even in a very short iframe viewport (px). */
const DROPDOWN_MIN_HEIGHT = 120;

/**
 * Natural (unconstrained) height of the dropdown: header + list (capped) + footer.
 * Measured from the rendered element because the list length varies with search.
 */
function measureDropdownHeight(dropdownEl: HTMLElement | null): number {
  if (!dropdownEl) return 0;
  const header = dropdownEl.querySelector<HTMLElement>('.var-picker-header');
  const body = dropdownEl.querySelector<HTMLElement>('.var-picker-body');
  const footer = dropdownEl.querySelector<HTMLElement>('.var-picker-footer');
  const bodyHeight = body ? Math.min(body.scrollHeight, DROPDOWN_MAX_BODY_HEIGHT) : 0;
  return (header?.offsetHeight ?? 0) + bodyHeight + (footer?.offsetHeight ?? 0) + 2;
}

/**
 * Position the dropdown inside the *iframe* viewport.
 *
 * The panel lives in a host-sized iframe whose height equals the panel content,
 * so a tall list opened near the bottom would otherwise be clipped by the
 * iframe edge. We therefore pick the side (below/above) with room for the whole
 * list, and cap `maxHeight` to the space actually available so the list scrolls
 * instead of overflowing.
 */
function calculateDropdownPosition(
  buttonEl: HTMLButtonElement | null,
  dropdownEl: HTMLElement | null,
): React.CSSProperties {
  if (!buttonEl) return { position: 'fixed', top: '0px', left: '0px', zIndex: 9999 };
  const btnRect = buttonEl.getBoundingClientRect();
  const viewportWidth = window.innerWidth || document.documentElement.clientWidth || 400;
  const viewportHeight = window.innerHeight || document.documentElement.clientHeight || 600;

  const dropdownWidth = Math.min(290, viewportWidth - VIEWPORT_MARGIN * 2);

  // Position horizontally: align with button's right edge by default
  let left = btnRect.right - dropdownWidth;

  // If overflowing left edge (x < 10px), align with button's left edge
  if (left < VIEWPORT_MARGIN) {
    left = Math.max(VIEWPORT_MARGIN, btnRect.left);
  }

  // Ensure dropdown stays fully within the right edge
  if (left + dropdownWidth > viewportWidth - VIEWPORT_MARGIN) {
    left = Math.max(VIEWPORT_MARGIN, viewportWidth - dropdownWidth - VIEWPORT_MARGIN);
  }

  const naturalHeight = measureDropdownHeight(dropdownEl) || DROPDOWN_MIN_HEIGHT;
  const spaceBelow = viewportHeight - btnRect.bottom - DROPDOWN_GAP - VIEWPORT_MARGIN;
  const spaceAbove = btnRect.top - DROPDOWN_GAP - VIEWPORT_MARGIN;

  // Flip above only when the list does not fit below and above has more room.
  const openAbove = naturalHeight > spaceBelow && spaceAbove > spaceBelow;
  const available = Math.max(DROPDOWN_MIN_HEIGHT, openAbove ? spaceAbove : spaceBelow);
  const maxHeight = Math.min(naturalHeight, available);

  let top = openAbove
    ? btnRect.top - DROPDOWN_GAP - maxHeight
    : btnRect.bottom + DROPDOWN_GAP;

  // Final clamp so rounding never pushes the dropdown past a viewport edge.
  top = Math.max(VIEWPORT_MARGIN, Math.min(top, viewportHeight - VIEWPORT_MARGIN - maxHeight));

  return {
    position: 'fixed',
    top: `${Math.round(top)}px`,
    left: `${Math.round(left)}px`,
    width: `${dropdownWidth}px`,
    maxWidth: `calc(100vw - ${VIEWPORT_MARGIN * 2}px)`,
    maxHeight: `${Math.round(maxHeight)}px`,
    zIndex: 9999,
  };
}

export function VariablePicker({
  variables = {},
  onSelect,
  inputRef,
  currentValue = '',
  onValueChange,
  title,
  variant = 'sm',
  label,
  className,
}: VariablePickerProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [search, setSearch] = useState('');
  const [dropdownStyle, setDropdownStyle] = useState<React.CSSProperties>({});

  const containerRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);

  const updatePosition = useCallback(() => {
    if (!buttonRef.current) return;
    setDropdownStyle(calculateDropdownPosition(buttonRef.current, dropdownRef.current));
  }, []);

  useEffect(() => {
    if (!isOpen) return;

    window.addEventListener('resize', updatePosition);
    window.addEventListener('scroll', updatePosition, true);

    const handleOutsideClick = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        containerRef.current &&
        !containerRef.current.contains(target) &&
        dropdownRef.current &&
        !dropdownRef.current.contains(target)
      ) {
        setIsOpen(false);
      }
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setIsOpen(false);
      }
    };

    document.addEventListener('mousedown', handleOutsideClick);
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('resize', updatePosition);
      window.removeEventListener('scroll', updatePosition, true);
      document.removeEventListener('mousedown', handleOutsideClick);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [isOpen, updatePosition]);

  // Reset search when closed
  useEffect(() => {
    if (!isOpen) {
      setSearch('');
    }
  }, [isOpen]);

  // Filter flow variables
  const filteredVariables = useMemo(() => {
    const entries = Object.entries(variables || {});
    if (!search.trim()) return entries;
    const q = search.toLowerCase();
    return entries.filter(([name, val]) => {
      return (
        name.toLowerCase().includes(q) ||
        String(val ?? '').toLowerCase().includes(q)
      );
    });
  }, [variables, search]);

  // Re-measure synchronously before paint: the list length changes with the
  // search term, and the dropdown must stay inside the (possibly short) viewport.
  useLayoutEffect(() => {
    if (!isOpen) return;
    updatePosition();
  }, [isOpen, updatePosition, search, filteredVariables.length]);

  const handleSelect = (varExpr: string) => {
    if (inputRef && inputRef.current !== undefined) {
      insertVariableIntoInput(inputRef.current, currentValue, varExpr, onValueChange);
    }
    onSelect(varExpr);
    setIsOpen(false);
  };

  const handleToggle = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!isOpen) {
      // Provisional coordinates (no dropdown to measure yet); the layout effect
      // re-measures with the real list height before the browser paints.
      setDropdownStyle(calculateDropdownPosition(buttonRef.current, null));
      setIsOpen(true);
    } else {
      setIsOpen(false);
    }
  };

  return (
    <div className="var-picker-wrapper" ref={containerRef}>
      <button
        ref={buttonRef}
        type="button"
        className={
          className ||
          (variant === 'icon'
            ? 'var-picker-btn-icon'
            : variant === 'badge'
              ? 'var-picker-btn-badge'
              : 'btn btn-sm var-picker-btn')
        }
        onClick={handleToggle}
        title={title || t('选择并插入流程变量', 'Select & Insert Variable')}
      >
        <IconBraces size={variant === 'icon' ? 14 : 12} />
        {variant !== 'icon' && (
          <span>{label || t('选变量', 'Variables')}</span>
        )}
      </button>

      {isOpen && (
        <div
          ref={dropdownRef}
          className="var-picker-dropdown"
          style={dropdownStyle}
          onClick={(e) => e.stopPropagation()}
        >
          <div className="var-picker-header">
            <div className="var-picker-search-box">
              <IconSearch size={13} className="text-muted" />
              <input
                ref={searchInputRef}
                autoFocus
                type="text"
                className="var-picker-search-input"
                placeholder={t('搜索可用变量...', 'Search variables...')}
                value={search}
                onChange={(e) => setSearch(e.target.value)}
              />
              {search && (
                <button
                  type="button"
                  className="var-picker-clear-btn"
                  onClick={() => setSearch('')}
                >
                  <IconX size={12} />
                </button>
              )}
            </div>
          </div>

          <div className="var-picker-body">
            {/* 1. Flow & Environment Variables */}
            <div className="var-picker-section">
              <div className="var-picker-section-title">
                <span className="row" style={{ gap: '4px' }}>
                  <IconGlobe size={11} />
                  <span>{t('可用流程变量', 'Available Flow Variables')}</span>
                </span>
                <span className="var-picker-count">{filteredVariables.length}</span>
              </div>

              {filteredVariables.length > 0 ? (
                <div className="var-picker-list">
                  {filteredVariables.map(([name, val]) => (
                    <button
                      key={name}
                      type="button"
                      className="var-picker-item"
                      onClick={() => handleSelect(name)}
                    >
                      <div className="var-picker-item-main">
                        <span className="var-picker-tag">{`{{${name}}}`}</span>
                        {val !== undefined && val !== null && val !== '' && (
                          <span className="var-picker-preview" title={String(val)}>
                            = {typeof val === 'object' ? JSON.stringify(val) : String(val)}
                          </span>
                        )}
                      </div>
                      <span className="var-picker-insert-hint">{t('插入', 'Insert')}</span>
                    </button>
                  ))}
                </div>
              ) : (
                <div className="var-picker-empty">
                  {search ? (
                    t('无匹配变量', 'No matching variables')
                  ) : (
                    <div>
                      <div>{t('当前流程暂无定义变量', 'No flow variables defined')}</div>
                      <div style={{ fontSize: '10px', marginTop: '4px', opacity: 0.8 }}>
                        {t(
                          '可在「环境管理」、流程初始变量或「变量提取」节点中定义',
                          'Define them in Environments, Initial Variables, or Variable Extractor nodes'
                        )}
                      </div>
                    </div>
                  )}
                </div>
              )}
            </div>

            {/* 2. Custom Expression Helper if user typed something */}
            {search.trim() && (
              <div className="var-picker-section">
                <button
                  type="button"
                  className="var-picker-item var-picker-custom"
                  onClick={() => handleSelect(search.trim())}
                >
                  <div className="var-picker-item-main">
                    <span className="var-picker-tag">{`{{${search.trim()}}}`}</span>
                    <span className="var-picker-preview">{t('自定义变量名', 'Custom variable name')}</span>
                  </div>
                  <span className="var-picker-insert-hint">{t('插入', 'Insert')}</span>
                </button>
              </div>
            )}
          </div>

          <div className="var-picker-footer">
            <span className="hint">
              {t(
                '支持流程变量、环境变量与变量提取节点定义的变量',
                'Supports Flow, Environment, and Extracted variables'
              )}
            </span>
          </div>
        </div>
      )}
    </div>
  );
}
