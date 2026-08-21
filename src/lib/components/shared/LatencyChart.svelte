<script lang="ts">
  import type { ECharts } from "echarts/core";
  import { onMount } from "svelte";
  import { locale, t } from "@i18n";

  let { rttMs } = $props<{ rttMs: number | null }>();
  let element = $state<HTMLDivElement | null>(null);
  let chart: ECharts | null = null;
  const samples: Array<{ time: number; value: number }> = [];
  const maxSamples = 60;
  let intervalId: number | null = null;
  let resizeObserver: ResizeObserver | null = null;
  let themeObserver: MutationObserver | null = null;

  const currentLatency = $derived(rttMs == null ? "--" : `${rttMs} ms`);

  $effect(() => {
    void $locale;
    render();
  });

  async function initializeChart(
    chartElement: HTMLDivElement,
    isMounted: () => boolean,
  ): Promise<void> {
    const [
      { init, use },
      { LineChart },
      { GridComponent, MarkAreaComponent, TooltipComponent },
      { CanvasRenderer },
    ] = await Promise.all([
      import("echarts/core"),
      import("echarts/charts"),
      import("echarts/components"),
      import("echarts/renderers"),
    ]);
    if (!isMounted()) return;

    use([CanvasRenderer, GridComponent, LineChart, MarkAreaComponent, TooltipComponent]);
    chart = init(chartElement, undefined, { renderer: "canvas" });
    render();
  }

  function cssVariable(name: string): string {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  }

  function render(): void {
    if (!chart) return;
    const values = samples.map((sample) => sample.value);
    const range = Math.max(...values, 0) - Math.min(...values, 0);
    const max = Math.max(250, Math.max(...values, 0) + Math.max(5, Math.ceil(range * 0.25)));
    const primary = cssVariable("--primary-solid") || cssVariable("--primary");
    const border = cssVariable("--border");
    const muted = cssVariable("--muted");

    chart.setOption({
      animation: !window.matchMedia("(prefers-reduced-motion: reduce)").matches,
      animationDuration: 220,
      animationDurationUpdate: 260,
      animationEasingUpdate: "cubicOut",
      grid: { top: 14, right: 12, bottom: 18, left: 42 },
      tooltip: {
        trigger: "axis",
        backgroundColor: cssVariable("--surface"),
        borderColor: border,
        borderWidth: 1,
        textStyle: { color: cssVariable("--text"), fontSize: 12 },
        valueFormatter: (value: number | string) => `${value} ms`,
      },
      xAxis: {
        type: "time",
        boundaryGap: false,
        axisLine: { lineStyle: { color: border } },
        axisTick: { show: false },
        axisLabel: { show: false },
        splitLine: { show: false },
      },
      yAxis: {
        type: "value",
        min: 0,
        max,
        axisLabel: { color: muted, fontSize: 11, formatter: "{value} ms" },
        axisLine: { show: false },
        axisTick: { show: false },
        splitLine: { lineStyle: { color: border, type: "dashed" } },
      },
      series: [
        {
          type: "line",
          data: samples.map((sample) => [sample.time, sample.value]),
          smooth: 0.35,
          showSymbol: false,
          lineStyle: { color: primary, width: 2 },
          itemStyle: { color: primary },
          areaStyle: { color: primary, opacity: 0.06 },
          markArea: {
            silent: true,
            label: { position: "insideRight", color: muted, fontSize: 10 },
            data: [
              [
                {
                  name: t("join.lowLatency", { threshold: 80 }),
                  yAxis: 0,
                  itemStyle: { color: cssVariable("--success"), opacity: 0.12 },
                },
                { yAxis: 80 },
              ],
              [
                {
                  name: t("join.mediumLatency", { low: 80, high: 180 }),
                  yAxis: 80,
                  itemStyle: { color: cssVariable("--warning"), opacity: 0.12 },
                },
                { yAxis: 180 },
              ],
              [
                {
                  name: t("join.highLatency", { threshold: 180 }),
                  yAxis: 180,
                  itemStyle: { color: cssVariable("--danger"), opacity: 0.1 },
                },
                { yAxis: max },
              ],
            ],
          },
        },
      ],
    });
  }

  function recordSample(): void {
    if (rttMs == null) return;
    samples.push({ time: Date.now(), value: rttMs });
    if (samples.length > maxSamples) samples.shift();
    render();
  }

  onMount(() => {
    if (!element) return;
    const chartElement = element;
    let mounted = true;
    void initializeChart(chartElement, () => mounted).catch((error: unknown) => {
      console.error("Failed to load latency chart", error);
    });
    recordSample();
    intervalId = window.setInterval(recordSample, 1000);
    resizeObserver = new ResizeObserver(() => chart?.resize());
    resizeObserver.observe(chartElement);
    themeObserver = new MutationObserver(render);
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class", "data-theme", "style"],
    });
    return () => {
      mounted = false;
      if (intervalId != null) window.clearInterval(intervalId);
      resizeObserver?.disconnect();
      themeObserver?.disconnect();
      chart?.dispose();
    };
  });
</script>

<section class="latency-chart" aria-label={t("join.latencyHistory")}>
  <div class="latency-chart-heading">
    <span>{t("join.latencyHistory")}</span><strong>{currentLatency}</strong>
  </div>
  <div bind:this={element} class="latency-chart-canvas"></div>
</section>
