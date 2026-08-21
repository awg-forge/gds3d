<script lang="ts">
  import { t } from "@i18n";
  import logo from "../../assets/logo.png";
  import Dialog from "./ui/Dialog.svelte";

  let { open = $bindable(false) }: { open?: boolean } = $props();
</script>

<Dialog bind:open title={t("gds.about")} closeLabel={t("gds.closeDialog")} width="560px">
  <div class="about-content">
    <img class="about-logo" src={logo} alt="gds3d" />
    <div class="about-copy">
      <h2>gds3d</h2>
      <p>{t("gds.aboutDescription")}</p>
      <small>{t("gds.version", { version: "0.1.4" })}</small>
    </div>
  </div>
</Dialog>

<style>
  .about-content {
    display: grid;
    grid-template-columns: 92px minmax(0, 1fr);
    align-items: center;
    gap: 24px;
    padding: 4px 0;
  }
  .about-logo {
    width: 92px;
    height: 92px;
    object-fit: contain;
  }
  .about-copy {
    min-width: 0;
  }
  h2 {
    margin: 0 0 9px;
    color: var(--text);
    font-size: 1.65rem;
    line-height: 1.1;
  }
  p {
    margin: 0;
    color: var(--muted);
    line-height: 1.55;
  }
  small {
    display: inline-block;
    margin-top: 15px;
    color: var(--muted);
    font-size: 0.78rem;
  }
  @media (max-width: 520px) {
    .about-content {
      grid-template-columns: 1fr;
      justify-items: center;
      text-align: center;
    }
  }
</style>
