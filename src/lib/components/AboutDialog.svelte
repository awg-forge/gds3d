<script lang="ts">
  import { onMount } from "svelte";
  import { getAppVersion } from "@api/app";
  import { t } from "@i18n";
  import logo from "../../assets/logo.png";
  import Dialog from "./ui/Dialog.svelte";

  let { open = $bindable(false) }: { open?: boolean } = $props();
  let version = $state("--");

  onMount(async () => {
    try {
      version = await getAppVersion();
    } catch {
      // The browser preview has no Tauri runtime.
    }
  });
</script>

<Dialog bind:open title={t("gds.about")} closeLabel={t("gds.closeDialog")} width="560px">
  <div class="about-content">
    <img class="about-logo" src={logo} alt="gds3d" />
    <div class="about-copy">
      <h2 aria-label="gds3d"><span>gds</span><strong>3d</strong></h2>
      <p>{t("gds.aboutDescription")}</p>
      <small>{t("gds.version", { version })}</small>
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
    font-family: "Gds3d Display", serif;
    font-size: 2.6rem;
    font-style: italic;
    font-weight: 500;
    line-height: 1;
    letter-spacing: -0.055em;
  }
  h2 span,
  h2 strong {
    display: inline-block;
  }
  h2 strong {
    margin-left: 0.08em;
    color: var(--primary);
    font-weight: 500;
    letter-spacing: -0.035em;
    transform: translateY(-0.04em) rotate(-2deg);
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
