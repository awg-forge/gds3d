import { toast } from "svelte-sonner";

export type ToastTone = "success" | "error" | "info";
export type ToastId = string | number;

export function showToast(message: string, tone: ToastTone = "info"): void {
  const options = { duration: tone === "error" ? 4000 : 2600 };
  if (tone === "success") toast.success(message, options);
  else if (tone === "error") toast.error(message, options);
  else toast.info(message, options);
}

export function showLoadingToast(message: string): ToastId {
  return toast.loading(message, { duration: Infinity });
}

export function finishToast(id: ToastId, message: string, tone: "success" | "error"): void {
  const options = { id, duration: tone === "error" ? 4000 : 2600 };
  if (tone === "success") toast.success(message, options);
  else toast.error(message, options);
}
