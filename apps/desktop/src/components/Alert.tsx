import type { ReactNode } from "react";

type Props = {
  kind?: "info" | "error" | "success" | "warn";
  children: ReactNode;
};

export function Alert({ kind = "info", children }: Props) {
  const cls = kind === "info" ? "alert" : `alert ${kind}`;
  return <div className={cls}>{children}</div>;
}
