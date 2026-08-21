<script lang="ts">
  import { onMount } from "svelte";
  import logoUrl from "../../assets/logo.png";
  import {
    checkForAppUpdate,
    getAppVersion,
    installAppUpdate,
    openProjectRepository,
    type AppUpdate,
  } from "@api/app";
  import { t } from "@i18n";
  import { Check, Download, ExternalLink, RefreshCw } from "@lucide/svelte";
  import { showToast } from "../state";

  let version = $state("--");
  let update = $state<AppUpdate | null>(null);
  let updateState = $state<"idle" | "checking" | "available" | "current" | "downloading" | "error">(
    "idle",
  );

  async function checkForUpdates(): Promise<void> {
    updateState = "checking";
    try {
      update = await checkForAppUpdate();
      updateState = update ? "available" : "current";
    } catch (error) {
      console.error("Failed to check for updates", error);
      updateState = "error";
      showToast(t("about.updateError"), "error");
    }
  }

  async function installUpdate(): Promise<void> {
    if (!update) return;
    updateState = "downloading";
    try {
      await installAppUpdate(update);
    } catch (error) {
      console.error("Failed to install update", error);
      updateState = "error";
      showToast(t("about.installUpdateError"), "error");
    }
  }

  onMount(async () => {
    try {
      version = await getAppVersion();
    } catch {
      /* Browser preview has no Tauri runtime. */
    }
  });
</script>

<section class="about-view">
  <div class="about-product">
    <img src={logoUrl} alt="" draggable="false" />
    <div>
      <h2>SeaLantern Connect</h2>
      <p>{t("about.description")}</p>
    </div>
  </div>
  <dl class="about-details">
    <div>
      <dt>{t("about.version")}</dt>
      <dd>v{version}</dd>
    </div>
    <div>
      <dt>{t("about.license")}</dt>
      <dd>Apache-2.0</dd>
    </div>
    <div>
      <dt>{t("about.developer")}</dt>
      <dd>SeaLantern-Studio</dd>
    </div>
    <div>
      <dt>{t("about.projectRepository")}</dt>
      <dd>
        <button
          class="about-link-button"
          type="button"
          onclick={() => void openProjectRepository()}
        >
          {t("about.openProject")}
          <ExternalLink size={14} strokeWidth={2} />
        </button>
      </dd>
    </div>
  </dl>
  <div class="about-update">
    <div>
      <h3>{t("about.updateTitle")}</h3>
      <p>
        {#if updateState === "available" && update}
          {t("about.updateAvailable", { version: update.version })}
        {:else if updateState === "current"}
          {t("about.upToDate")}
        {:else if updateState === "error"}
          {t("about.updateError")}
        {:else}
          {t("about.updateDescription")}
        {/if}
      </p>
    </div>
    {#if updateState === "available"}
      <button class="about-update-button" type="button" onclick={installUpdate}>
        <Download size={15} strokeWidth={2} />
        {t("about.installUpdate")}
      </button>
    {:else}
      <button
        class="about-update-button"
        type="button"
        onclick={checkForUpdates}
        disabled={updateState === "checking" || updateState === "downloading"}
      >
        {#if updateState === "current"}
          <Check size={15} strokeWidth={2} />
        {:else}
          <RefreshCw size={15} strokeWidth={2} />
        {/if}
        {updateState === "checking" ? t("about.checkingUpdates") : t("about.checkUpdates")}
      </button>
    {/if}
  </div>
  <div class="about-frp">
    <p>{t("about.frpDescription")}</p>
  </div>
</section>
