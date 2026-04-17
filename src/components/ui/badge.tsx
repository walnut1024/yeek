import { mergeProps } from "@base-ui/react/merge-props"
import { useRender } from "@base-ui/react/use-render"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "@/lib/utils"

const badgeVariants = cva(
  "group/badge inline-flex h-5 w-fit shrink-0 items-center justify-center gap-1 overflow-hidden rounded-sm border border-border px-1.5 py-0.5 text-[12px] font-medium whitespace-nowrap text-muted-foreground transition-colors focus-visible:border-ring focus-visible:ring-0 has-data-[icon=inline-end]:pr-1.5 has-data-[icon=inline-start]:pl-1.5 aria-invalid:border-destructive [&>svg]:pointer-events-none [&>svg]:size-3!",
  {
    variants: {
      variant: {
        default: "border-border bg-secondary text-foreground [a]:hover:bg-accent",
        secondary:
          "border-border bg-secondary text-muted-foreground [a]:hover:bg-accent",
        destructive:
          "border-[#4c2b2c] bg-destructive/10 text-destructive [a]:hover:bg-destructive/20",
        outline:
          "border-border bg-transparent text-muted-foreground [a]:hover:bg-accent [a]:hover:text-foreground",
        ghost:
          "border-transparent bg-transparent hover:bg-accent hover:text-foreground",
        link: "text-primary underline-offset-4 hover:underline",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  }
)

function Badge({
  className,
  variant = "default",
  render,
  ...props
}: useRender.ComponentProps<"span"> & VariantProps<typeof badgeVariants>) {
  return useRender({
    defaultTagName: "span",
    props: mergeProps<"span">(
      {
        className: cn(badgeVariants({ variant }), className),
      },
      props
    ),
    render,
    state: {
      slot: "badge",
      variant,
    },
  })
}

export { Badge, badgeVariants }
