import { toast } from "svelte-sonner";

export type ToastTone = "success" | "error" | "info";

export function showToast(message: string, tone: ToastTone = "info"): void {
  const options = { duration: tone === "error" ? 4000 : 2600 };
  if (tone === "success") toast.success(message, options);
  else if (tone === "error") toast.error(message, options);
  else toast.info(message, options);
}
