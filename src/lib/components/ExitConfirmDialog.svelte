<script lang="ts">
  import { t } from "@i18n";
  import Button from "./ui/Button.svelte";
  import Dialog from "./ui/Dialog.svelte";

  let {
    open = $bindable(false),
    busy = false,
    oncancel,
    ondiscard,
    onsave,
  }: {
    open?: boolean;
    busy?: boolean;
    oncancel: () => void;
    ondiscard: () => void;
    onsave: () => void;
  } = $props();
</script>

<Dialog
  bind:open
  title={t("gds.unsavedExitTitle")}
  closeLabel={t("gds.closeDialog")}
  showClose={false}
  width="440px"
>
  <p>{t("gds.unsavedExitMessage")}</p>
  {#snippet footer()}
    <Button variant="ghost" disabled={busy} onclick={oncancel}>{t("gds.cancel")}</Button>
    <Button variant="danger" disabled={busy} onclick={ondiscard}>{t("gds.discardChanges")}</Button>
    <Button loading={busy} onclick={onsave}>{t("gds.save")}</Button>
  {/snippet}
</Dialog>

<style>
  p {
    margin: 0;
    color: var(--text);
  }
</style>
