import * as React from "react"

import { cn } from "@/lib/utils"

interface SelectProps extends React.ComponentProps<"select"> {
  children: React.ReactNode;
}

function Select({ className, children, ...props }: SelectProps) {
  return (
    <select
      data-slot="select"
      className={cn(
        "placeholder:text-muted-foreground dark:bg-input/30 border-input h-9 w-full rounded-md border bg-transparent px-3 py-1 text-base shadow-xs transition-[color,box-shadow] outline-none disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 md:text-sm",
        "focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px]",
        "aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive",
        className
      )}
      {...props}
    >
      {children}
    </select>
  )
}

export { Select }
