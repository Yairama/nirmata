import { cva, type VariantProps } from "class-variance-authority";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}

const buttonStyles = cva("disabled:cursor-not-allowed disabled:border-line disabled:bg-subtle disabled:text-muted disabled:opacity-70", {
  variants: {
    variant: {
      primary: "",
      secondary: "secondary border-line bg-raised text-ink enabled:hover:border-line-strong enabled:hover:bg-subtle",
      ghost: "ghost border-line bg-transparent text-ink enabled:hover:border-line-strong enabled:hover:bg-subtle",
      danger: "danger border-danger bg-danger text-on-action",
      dangerOutline: "danger-outline border-danger bg-transparent text-danger enabled:hover:bg-danger-soft",
      icon: "icon-button size-9 min-h-9 shrink-0 rounded-full border-line bg-transparent p-0 text-muted enabled:hover:border-line-strong enabled:hover:bg-subtle enabled:hover:text-ink",
    },
    size: {
      normal: "",
      compact: "min-h-8 px-2.5 py-1 text-xs",
    },
  },
  defaultVariants: {
    variant: "primary",
    size: "normal",
  },
});

const chipStyles = cva(
  "inline-flex min-h-6 items-center rounded-full border border-line bg-subtle px-2 py-0.5 text-xs font-semibold text-ink",
  {
    variants: {
      kind: {
        badge: "badge",
        status: "status-chip",
        readOnly: "read-only-chip",
        count: "count-badge ml-1 min-w-6 justify-center border-0 bg-subtle px-1.5",
      },
      tone: {
        neutral: "",
        error: "error border-danger bg-danger-soft text-danger",
        warning: "warning border-warning bg-warning-soft text-warning",
        conflict: "conflict border-conflict bg-conflict-soft text-conflict",
        info: "info border-info bg-info-soft text-info",
        success: "ready success border-success bg-success-soft text-success",
        perspective: "context border-perspective bg-perspective-soft text-perspective",
        kind: "kind bg-raised",
      },
    },
    defaultVariants: {
      kind: "badge",
      tone: "neutral",
    },
  },
);

const noticeStyles = cva("notice grid gap-2 rounded-xl border border-line bg-surface p-4 text-sm", {
  variants: {
    tone: {
      neutral: "",
      warning: "warning border-warning bg-warning-soft",
      info: "info border-info bg-info-soft",
      error: "error border-danger bg-danger-soft",
    },
  },
  defaultVariants: {
    tone: "neutral",
  },
});

type ButtonStyleProps = VariantProps<typeof buttonStyles>;
type ChipStyleProps = VariantProps<typeof chipStyles>;
type NoticeStyleProps = VariantProps<typeof noticeStyles>;

export { buttonStyles, chipStyles, cn, noticeStyles };
export type { ButtonStyleProps, ChipStyleProps, NoticeStyleProps };
