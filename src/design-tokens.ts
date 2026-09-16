/** Focusly 设计令牌中心：设计系统常量的唯一来源。
 *  组件内联样式需要精确数值时从这里引用，避免散落的魔法数字；
 *  Tailwind 类（rounded-md / z-50 等）仍以 index.css 的 @theme 令牌为准，两者语义保持一致。 */

/** 间距（基于 Tailwind 4px 网格） */
export const spacing = { xs: 4, sm: 8, md: 16, lg: 24, xl: 32 } as const;

/** 圆角 */
export const radius = { sm: 6, md: 8, lg: 12, xl: 16, full: 9999 } as const;

/** 动效时长（ms） */
export const duration = { fast: 150, normal: 200, slow: 300 } as const;

/** easing 曲线 */
export const easing = {
  standard: "cubic-bezier(0.2, 0, 0, 1)",
  decelerate: "cubic-bezier(0, 0, 0, 1)",
} as const;

/** z-index 层级（与 Tailwind z-* 惯例对齐：dropdown 50 < overlay 90 < sticky/modal/toast 100 < tooltip 110） */
export const zIndex = {
  base: 0,
  dropdown: 50,
  sticky: 100,
  overlay: 90,
  modal: 100,
  toast: 100,
  tooltip: 110,
} as const;

/** 断点（Tailwind 默认） */
export const breakpoint = { sm: 640, md: 768, lg: 1024, xl: 1280 } as const;
